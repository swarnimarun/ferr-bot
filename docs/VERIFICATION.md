# Local verification — 2026-11-01

Base: real public Git clone of swarnimarun/ferr-bot, master commit `e73db16f5af3d84f447ce132cf1791742f8ba974`. Original LICENSE unchanged. No remote mutation, PR, deployment, Telegram token, Telegram message, or homelab contact occurred.

Compiler: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, stable x86_64-unknown-linux-gnu, official rustup in isolated writable directory. Dependency source: crates.io; exact resolution in Cargo.lock.

Final checks passed:

- `cargo fmt --check`
- `cargo clippy --locked --offline --all-targets -- -D warnings`
- `cargo test --locked --offline`: 18 unit tests passed; zero failed; binary/doc test targets had no tests.
- `cargo run --locked --offline -- demo`: successful simulated status → approval → consume → audit sequence; transcript in DEMO.txt.
- `git diff --check`
- Independent reviewer reran all 18 library tests and inspected authorization, approval ledger, transport and composite target binding.

Covered: owner/private-chat deny-default behavior, extra arguments/unknown commands, invalid/duplicate inventory and connector rejection, single-use flow, cross-lab target move, malformed callbacks, exact expiry, wrong actor/chat/target, unknown approval, pre-creation time, durable reopen, concurrent consumption, request idempotency, immutability, failed-audit rollback, monotonic cursor, token-free audit display, token URL injection, missing Telegram sender.

Not verified: live Telegram API behavior/end-to-end delivery, actual homelab integration, real actions, production deployment, remote CI, long-duration operational behavior, external vulnerability database audit. No scheduler or real connectors exist. Transport exits safely on network/storage error; an interrupted delivery can lose or duplicate a reply, but cannot repeat a completed simulated operation. State is trusted local SQLite, not a tamper-proof audit service. This is an adapter-neutral functional foundation, not a production fleet manager.

## Follow-up verification — 2026-11-02

After importing the foundation into this repository, `cargo fmt --check`,
`cargo clippy --locked --offline --all-targets -- -D warnings`,
`cargo test --locked --offline`, and `cargo run --locked --offline -- demo`
passed locally. The suite now contains 20 tests, including cursor reset after
prolonged inactivity and transactional rollback of cursor updates. Live
Telegram and real services remain untested.
