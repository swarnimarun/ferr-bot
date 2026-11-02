use crate::{
    inventory::Inventory,
    store::{Action, Store},
};

#[derive(Clone, Copy)]
pub struct Identity {
    pub actor: i64,
    pub chat: i64,
    pub private: bool,
}
pub struct Reply {
    pub text: String,
    pub approve: Option<String>,
}
pub struct Control {
    owner: i64,
    pub inventory: Inventory,
    pub store: Store,
}
impl Control {
    pub fn new(owner: i64, inventory: Inventory, store: Store) -> Result<Self, &'static str> {
        if owner <= 0 {
            return Err("owner must be a positive numeric Telegram user id");
        }
        Ok(Self {
            owner,
            inventory,
            store,
        })
    }
    pub fn authorized(&self, who: Identity) -> bool {
        who.private && who.actor == self.owner && who.chat == self.owner
    }
    pub fn command(
        &mut self,
        who: Identity,
        update_id: i64,
        text: &str,
        now: i64,
    ) -> Option<Reply> {
        if !self.authorized(who) {
            return None;
        }
        let parts: Vec<_> = text.split_whitespace().collect();
        let response = match parts.as_slice() {
            ["/start"] | ["/help"] => "Commands: /services /status /digest /status SERVICE /restart SERVICE /audit\nAll data and actions are SIMULATED. Restart requires confirmation within 120 seconds.".into(),
            ["/services"] | ["/status"] | ["/digest"] => self.inventory.digest(),
            ["/status", id] => self.inventory.find(id).map(|s| format!("SIMULATED {}/{}: {:?}", s.lab,s.id,s.status)).unwrap_or_else(|| "Unknown service.".into()),
            ["/restart", id] if self.inventory.find(id).is_some() => {
                return Some(match self.store.request(update_id, who.actor, who.chat, Action::SimulateRestart, &format!("{}.{}", self.inventory.find(id).unwrap().lab, id), now, 120) {
                    Ok(a) => Reply { text: format!("Confirm SIMULATED restart of {}? No host will be contacted. Expires at Unix {}. Do nothing to cancel.", a.service, a.expires_at), approve: Some(format!("approve:{}", a.id)) },
                    Err(_) => reply("Request unavailable; no action performed."),
                });
            }
            ["/audit"] => self.store.audit(10).map(|entries| if entries.is_empty() { "No audit entries.".into() } else { entries.join("\n") }).unwrap_or_else(|_| "Audit unavailable.".into()),
            _ => "Unknown command or service. Use /help. No action performed.".into(),
        };
        Some(Reply {
            text: response,
            approve: None,
        })
    }
    pub fn callback(&mut self, who: Identity, data: &str, now: i64) -> Option<Reply> {
        if !self.authorized(who) {
            return None;
        }
        let outcome = (|| {
            let id = data
                .strip_prefix("approve:")
                .filter(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or(())?;
            let a = self.store.get(id).map_err(|_| ())?;
            if !self
                .inventory
                .services
                .iter()
                .any(|s| format!("{}.{}", s.lab, s.id) == a.service)
            {
                return Err(());
            }
            self.store
                .consume(
                    id,
                    who.actor,
                    who.chat,
                    Action::SimulateRestart,
                    &a.service,
                    now,
                )
                .map_err(|_| ())?;
            Ok(a.service)
        })();
        Some(match outcome {
            Ok(service) => reply(&format!(
                "SIMULATED restart recorded for {service}. No host contacted."
            )),
            Err(()) => {
                reply("Approval rejected, expired, or already used. No new action performed.")
            }
        })
    }
}
fn reply(text: &str) -> Reply {
    Reply {
        text: text.into(),
        approve: None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn control() -> Control {
        Control::new(
            42,
            Inventory::parse(include_str!("../config/inventory.example.json")).unwrap(),
            Store::open(":memory:").unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn deny_default_identity() {
        let mut c = control();
        for who in [
            Identity {
                actor: 1,
                chat: 42,
                private: true,
            },
            Identity {
                actor: 42,
                chat: 43,
                private: true,
            },
            Identity {
                actor: 42,
                chat: 42,
                private: false,
            },
        ] {
            assert!(c.command(who, 1, "/status", 100).is_none());
            assert!(c.callback(who, "approve:abcd", 100).is_none());
        }
    }
    #[test]
    fn approval_flow_is_single_use() {
        let mut c = control();
        let who = Identity {
            actor: 42,
            chat: 42,
            private: true,
        };
        let request = c.command(who, 1, "/restart media", 100).unwrap();
        let data = request.approve.unwrap();
        assert!(c
            .callback(who, &data, 101)
            .unwrap()
            .text
            .starts_with("SIMULATED restart recorded"));
        assert!(c
            .callback(who, &data, 102)
            .unwrap()
            .text
            .starts_with("Approval rejected"));
    }
    #[test]
    fn unknown_or_extra_arguments_never_create_approval() {
        let mut c = control();
        let who = Identity {
            actor: 42,
            chat: 42,
            private: true,
        };
        for text in ["/restart missing", "/restart media ; rm", "/shell ls"] {
            assert!(c.command(who, 1, text, 100).unwrap().approve.is_none());
        }
    }
    #[test]
    fn moving_service_to_another_lab_invalidates_approval() {
        let mut c = control();
        let who = Identity {
            actor: 42,
            chat: 42,
            private: true,
        };
        let data = c
            .command(who, 1, "/restart media", 100)
            .unwrap()
            .approve
            .unwrap();
        c.inventory.services[0].lab = "different".into();
        assert!(c
            .callback(who, &data, 101)
            .unwrap()
            .text
            .starts_with("Approval rejected"));
        assert_eq!(c.store.audit_count().unwrap(), 1);
    }
    #[test]
    fn malformed_callbacks_and_expiry_are_denied() {
        let mut c = control();
        let who = Identity {
            actor: 42,
            chat: 42,
            private: true,
        };
        let data = c
            .command(who, 1, "/restart media", 100)
            .unwrap()
            .approve
            .unwrap();
        for bad in ["approve:zzzz", "shell:foo", "approve:"] {
            assert!(c
                .callback(who, bad, 101)
                .unwrap()
                .text
                .starts_with("Approval rejected"));
        }
        assert!(c
            .callback(who, &data, 220)
            .unwrap()
            .text
            .starts_with("Approval rejected"));
        assert_eq!(c.store.audit_count().unwrap(), 1);
    }
}
