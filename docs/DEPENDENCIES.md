# Dependency choice and provenance

Official documentation checked during implementation (2026-11-01):

- https://core.telegram.org/bots/api — HTTPS Bot API, getUpdates offsets, callback acknowledgments.
- https://docs.rs/reqwest/latest/reqwest/ — blocking client supported; this foundation uses the 0.12 compatibility line with Rustls and JSON.
- https://docs.rs/rusqlite/0.37.0/rusqlite/ — SQLite transactions, bound parameters and bundled SQLite support.
- https://docs.rs/serde/latest/serde/ — typed serialization/deserialization.

Cargo resolved and downloaded official registry packages; Cargo.lock records exact versions/checksums. Top-level resolution: reqwest 0.12.28, rusqlite 0.37.0, serde 1.0.229, serde_json 1.0.154, thiserror 2.0.21. These are intentionally compatible version lines, not a claim that all are the latest releases. Old telegram-bot 0.6/tokio-core/futures 0.1 dependencies have been removed.

Local compiler: rustc 1.99.1, installed using official rustup into an isolated writable toolchain directory. CI runs a runner-provided Rust toolchain and locked dependencies. No remote CI run has occurred. This work includes tests and review, not a vulnerability database audit or penetration test.
