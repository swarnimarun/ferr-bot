//! Durable single-use approval ledger. No external action is dispatched here.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, time::Duration};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    SimulateRestart,
}
impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SimulateRestart => "simulate_restart",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    pub id: String,
    pub actor: i64,
    pub chat: i64,
    pub action: Action,
    pub service: String,
    pub expires_at: i64,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("approval is unavailable, expired, consumed, or does not match")]
    Denied,
    #[error("invalid approval request")]
    Invalid,
    #[error("durable storage failed")]
    Database(#[from] rusqlite::Error),
}

pub struct Store {
    conn: Connection,
}
impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS approvals (
               id TEXT PRIMARY KEY, update_id INTEGER NOT NULL UNIQUE,
               actor INTEGER NOT NULL, chat INTEGER NOT NULL,
               action TEXT NOT NULL CHECK(action='simulate_restart'),
               service TEXT NOT NULL, parameters TEXT NOT NULL CHECK(parameters='{}'),
               created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
               consumed_at INTEGER,
               CHECK(expires_at > created_at)
             );
             CREATE TABLE IF NOT EXISTS audit (
               sequence INTEGER PRIMARY KEY AUTOINCREMENT,
               approval_id TEXT NOT NULL REFERENCES approvals(id),
               event TEXT NOT NULL CHECK(event IN ('requested','simulated_completed')),
               at INTEGER NOT NULL,
               UNIQUE(approval_id,event)
             );
             CREATE TABLE IF NOT EXISTS poll_state (
               singleton INTEGER PRIMARY KEY CHECK(singleton=1), next_offset INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO poll_state VALUES(1,0);
             CREATE TABLE IF NOT EXISTS poll_activity (
               singleton INTEGER PRIMARY KEY CHECK(singleton=1), last_update INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO poll_activity VALUES(1,0);
             CREATE TRIGGER IF NOT EXISTS approval_immutable
             BEFORE UPDATE OF id,update_id,actor,chat,action,service,parameters,created_at,expires_at ON approvals
             BEGIN SELECT RAISE(ABORT,'immutable approval'); END;"
        )?;
        Ok(Self { conn })
    }

    /// A replay of the same update returns the same request, never extends its lifetime.
    /// Reusing an update ID with different immutable fields fails closed.
    #[allow(clippy::too_many_arguments)]
    pub fn request(
        &mut self,
        update_id: i64,
        actor: i64,
        chat: i64,
        action: Action,
        service: &str,
        now: i64,
        ttl_seconds: i64,
    ) -> Result<Approval, StoreError> {
        if actor <= 0
            || chat <= 0
            || update_id < 0
            || now < 0
            || !(1..=300).contains(&ttl_seconds)
            || service.is_empty()
            || service.len() > 81
            || !service
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(StoreError::Invalid);
        }
        let expires = now.checked_add(ttl_seconds).ok_or(StoreError::Invalid)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM approvals WHERE update_id=?1",
                [update_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            let approval = read_approval(&tx, &id)?;
            if approval.actor != actor
                || approval.chat != chat
                || approval.action != action
                || approval.service != service
            {
                return Err(StoreError::Denied);
            }
            tx.commit()?;
            return Ok(approval);
        }
        let id: String = tx.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
        tx.execute("INSERT INTO approvals(id,update_id,actor,chat,action,service,parameters,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,'{}',?7,?8)",
            params![id,update_id,actor,chat,action.as_str(),service,now,expires])?;
        tx.execute(
            "INSERT INTO audit(approval_id,event,at) VALUES(?1,'requested',?2)",
            params![id, now],
        )?;
        let approval = read_approval(&tx, &id)?;
        tx.commit()?;
        Ok(approval)
    }

    /// Returns immutable fields, including for a consumed request. Never authorizes execution.
    pub fn get(&self, id: &str) -> Result<Approval, StoreError> {
        read_approval(&self.conn, id)
    }

    /// Consumes exactly once and records the mock result in the same transaction.
    /// This API must not be reused as an exactly-once guarantee for external side effects.
    pub fn consume(
        &mut self,
        id: &str,
        actor: i64,
        chat: i64,
        action: Action,
        service: &str,
        now: i64,
    ) -> Result<Approval, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE approvals SET consumed_at=?1 WHERE id=?2 AND actor=?3 AND chat=?4
             AND action=?5 AND service=?6 AND parameters='{}' AND created_at<=?1
             AND expires_at>?1 AND consumed_at IS NULL",
            params![now, id, actor, chat, action.as_str(), service],
        )?;
        if changed != 1 {
            return Err(StoreError::Denied);
        }
        tx.execute(
            "INSERT INTO audit(approval_id,event,at) VALUES(?1,'simulated_completed',?2)",
            params![id, now],
        )?;
        let approval = read_approval(&tx, id)?;
        tx.commit()?;
        Ok(approval)
    }

    pub fn offset(&self) -> Result<i64, StoreError> {
        Ok(self.conn.query_row(
            "SELECT next_offset FROM poll_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn set_offset(&mut self, offset: i64) -> Result<(), StoreError> {
        if offset < 0 {
            return Err(StoreError::Invalid);
        }
        self.conn.execute(
            "UPDATE poll_state SET next_offset=max(next_offset,?1) WHERE singleton=1",
            [offset],
        )?;
        Ok(())
    }
    /// Reset the cursor before Telegram may randomize update IDs after a week of silence.
    pub fn poll_offset(&mut self, now: i64) -> Result<i64, StoreError> {
        const IDLE_RESET_SECONDS: i64 = 6 * 24 * 60 * 60;
        if now < 0 {
            return Err(StoreError::Invalid);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let last: i64 = tx.query_row(
            "SELECT last_update FROM poll_activity WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        if last == 0 || now.saturating_sub(last) >= IDLE_RESET_SECONDS {
            tx.execute("UPDATE poll_state SET next_offset=0 WHERE singleton=1", [])?;
            tx.execute(
                "UPDATE poll_activity SET last_update=?1 WHERE singleton=1",
                [now],
            )?;
        }
        let offset = tx.query_row(
            "SELECT next_offset FROM poll_state WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(offset)
    }
    /// Persist a processed update and its time in the same transaction.
    pub fn record_update(&mut self, offset: i64, now: i64) -> Result<(), StoreError> {
        if offset < 0 || now < 0 {
            return Err(StoreError::Invalid);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE poll_state SET next_offset=max(next_offset,?1) WHERE singleton=1",
            [offset],
        )?;
        tx.execute(
            "UPDATE poll_activity SET last_update=?1 WHERE singleton=1",
            [now],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Fixed-format audit summaries; never contains credentials or free-form parameters.
    pub fn audit(&self, limit: u32) -> Result<Vec<String>, StoreError> {
        let mut stmt=self.conn.prepare("SELECT audit.sequence,audit.event,audit.at,approvals.action,approvals.service FROM audit JOIN approvals ON approvals.id=audit.approval_id ORDER BY audit.sequence DESC LIMIT ?1")?;
        let rows = stmt.query_map([limit.min(50)], |r| {
            Ok(format!(
                "{} {} at={} action={} service={}",
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?
            ))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
    pub fn audit_count(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM audit", [], |r| r.get(0))?)
    }
}

fn read_approval(conn: &Connection, id: &str) -> Result<Approval, StoreError> {
    let row = conn
        .query_row(
            "SELECT id,actor,chat,action,service,expires_at FROM approvals WHERE id=?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::Denied)?;
    if row.3 != Action::SimulateRestart.as_str() {
        return Err(StoreError::Denied);
    }
    Ok(Approval {
        id: row.0,
        actor: row.1,
        chat: row.2,
        action: Action::SimulateRestart,
        service: row.4,
        expires_at: row.5,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(s: &mut Store) -> Approval {
        s.request(1, 7, 7, Action::SimulateRestart, "lab-a.web", 100, 60)
            .unwrap()
    }
    fn consume(s: &mut Store, a: &Approval, now: i64) -> Result<Approval, StoreError> {
        s.consume(&a.id, 7, 7, Action::SimulateRestart, "lab-a.web", now)
    }
    fn disk_path() -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "ferr-store-{}-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    fn cleanup(p: &Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", p.display()));
        }
    }
    #[test]
    fn replay_and_binding_are_denied() {
        let mut s = Store::open(":memory:").unwrap();
        let a = request(&mut s);
        assert!(s.consume(&a.id, 8, 7, a.action, &a.service, 101).is_err());
        assert!(s.consume(&a.id, 7, 8, a.action, &a.service, 101).is_err());
        assert!(s.consume(&a.id, 7, 7, a.action, "different", 101).is_err());
        assert!(s
            .consume("unknown", 7, 7, a.action, &a.service, 101)
            .is_err());
        assert!(consume(&mut s, &a, 99).is_err());
        assert!(consume(&mut s, &a, 101).is_ok());
        assert!(consume(&mut s, &a, 102).is_err());
        assert_eq!(s.audit_count().unwrap(), 2);
    }
    #[test]
    fn exact_expiry_is_denied() {
        let mut s = Store::open(":memory:").unwrap();
        let a = request(&mut s);
        assert!(consume(&mut s, &a, 160).is_err());
        assert_eq!(s.audit_count().unwrap(), 1);
    }
    #[test]
    fn idempotency_does_not_extend_or_rebind() {
        let mut s = Store::open(":memory:").unwrap();
        let a = request(&mut s);
        assert_eq!(
            s.request(1, 7, 7, a.action, &a.service, 130, 300).unwrap(),
            a
        );
        assert!(s.request(1, 8, 7, a.action, &a.service, 100, 60).is_err());
        assert!(s.request(1, 7, 7, a.action, "other", 100, 60).is_err());
        assert_eq!(s.audit_count().unwrap(), 1);
    }
    #[test]
    fn survives_reopen() {
        let p = disk_path();
        let a;
        {
            let mut s = Store::open(&p).unwrap();
            a = request(&mut s);
            s.set_offset(42).unwrap();
        }
        {
            let mut s = Store::open(&p).unwrap();
            assert_eq!(s.get(&a.id).unwrap(), a);
            assert_eq!(s.offset().unwrap(), 42);
            consume(&mut s, &a, 110).unwrap();
        }
        {
            let mut s = Store::open(&p).unwrap();
            assert!(consume(&mut s, &a, 111).is_err());
            assert_eq!(s.audit_count().unwrap(), 2);
        }
        cleanup(&p);
    }
    #[test]
    fn concurrent_consume_only_one_wins() {
        let p = disk_path();
        let mut s = Store::open(&p).unwrap();
        let a = request(&mut s);
        drop(s);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let p = p.clone();
                let a = a.clone();
                let b = barrier.clone();
                std::thread::spawn(move || {
                    let mut s = Store::open(p).unwrap();
                    b.wait();
                    consume(&mut s, &a, 110).is_ok()
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| usize::from(h.join().unwrap()))
                .sum::<usize>(),
            1
        );
        cleanup(&p);
    }
    #[test]
    fn failed_audit_rolls_back_consumption() {
        let mut s = Store::open(":memory:").unwrap();
        let a = request(&mut s);
        s.conn.execute_batch("CREATE TRIGGER fail_audit BEFORE INSERT ON audit BEGIN SELECT RAISE(ABORT,'failed'); END;").unwrap();
        assert!(consume(&mut s, &a, 110).is_err());
        s.conn.execute_batch("DROP TRIGGER fail_audit;").unwrap();
        assert!(consume(&mut s, &a, 111).is_ok());
    }
    #[test]
    fn immutable_fields_and_unknown_actions_rejected() {
        let mut s = Store::open(":memory:").unwrap();
        let a = request(&mut s);
        assert!(s
            .conn
            .execute("UPDATE approvals SET service='other' WHERE id=?1", [&a.id])
            .is_err());
        assert!(s
            .conn
            .execute("UPDATE approvals SET action='shell' WHERE id=?1", [&a.id])
            .is_err());
    }
    #[test]
    fn audit_output_omits_approval_tokens() {
        let mut s = Store::open(":memory:").unwrap();
        let a = request(&mut s);
        let rows = s.audit(100).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].contains(&a.id));
        assert!(rows[0].contains("service=lab-a.web"));
    }
    #[test]
    fn invalid_requests_and_offset_regression() {
        let mut s = Store::open(":memory:").unwrap();
        assert!(s
            .request(1, 0, 7, Action::SimulateRestart, "ok", 100, 60)
            .is_err());
        assert!(s
            .request(1, 7, 7, Action::SimulateRestart, ";sh", 100, 60)
            .is_err());
        assert!(s
            .request(1, 7, 7, Action::SimulateRestart, "ok", 100, 301)
            .is_err());
        s.set_offset(10).unwrap();
        s.set_offset(2).unwrap();
        assert_eq!(s.offset().unwrap(), 10);
    }
    #[test]
    fn idle_cursor_reset_survives_restart_and_accepts_lower_ids() {
        let p = disk_path();
        {
            let mut s = Store::open(&p).unwrap();
            assert_eq!(s.poll_offset(100).unwrap(), 0);
            s.record_update(1000, 100).unwrap();
            assert_eq!(s.poll_offset(100 + 6 * 86400 - 1).unwrap(), 1000);
        }
        {
            let mut s = Store::open(&p).unwrap();
            assert_eq!(s.poll_offset(100 + 6 * 86400).unwrap(), 0);
            s.record_update(20, 100 + 6 * 86400).unwrap();
            s.record_update(10, 101 + 6 * 86400).unwrap();
            assert_eq!(s.poll_offset(101 + 6 * 86400).unwrap(), 20);
        }
        cleanup(&p);
    }
    #[test]
    fn cursor_and_activity_update_roll_back_together() {
        let mut s = Store::open(":memory:").unwrap();
        s.poll_offset(100).unwrap();
        s.conn.execute_batch("CREATE TRIGGER fail_activity BEFORE UPDATE ON poll_activity BEGIN SELECT RAISE(ABORT,'failed'); END;").unwrap();
        assert!(s.record_update(42, 101).is_err());
        assert_eq!(s.offset().unwrap(), 0);
    }
}
