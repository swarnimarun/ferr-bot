# ferr-bot — homelab control foundation

A Rust Telegram control plane for multiple labs and services. **This increment is mock-only:** inventory, health and restart results are simulated. It does not connect to hosts, run commands, mount Docker sockets, or change services.

## Try it without credentials or network

With a current stable Rust toolchain and C compiler (SQLite is bundled):

```sh
cargo build --locked
cargo test --locked
cargo run --locked -- demo
```

After the first dependency download, `cargo test --locked --offline` and `cargo run --locked --offline -- demo` work offline. The demo uses an in-memory database, a fictional owner, three fictional services and an approval/confirmation/audit sequence. It does not use a Telegram token or send messages.

## Telegram mode (manual opt-in; not deployed)

1. Obtain your own bot token and numeric Telegram user ID through trusted Telegram tooling. Never commit the token or paste it into issues/logs.
2. Create a private state directory and configure environment variables using your secret manager. Do not put token values in shell history. `FERR_OWNER_ID` is your user ID, **not** the bot ID.
3. Run one instance per bot and state database:

```sh
umask 077
mkdir -p state
# Set TELEGRAM_BOT_TOKEN securely; intentionally no example secret.
export FERR_OWNER_ID='<your-positive-numeric-user-id>'
export FERR_DB='state/ferr-bot.sqlite'
export FERR_INVENTORY='config/inventory.example.json'
cargo run --locked --release -- telegram
```

Only the configured owner in a private chat whose chat ID equals that owner is served. Groups, channels, other users, missing senders and bots are ignored. No discovery-by-first-message or `/setowner` path exists. Stop with Ctrl-C. Network and cursor storage errors stop the process with a sanitized error; approval and audit storage errors produce a generic failure reply. Run under a supervisor if desired, but there is no supplied deployment or automatic restart setup.

Commands: `/help`, `/services`, `/status`, `/status SERVICE`, `/digest`, `/restart SERVICE`, `/audit`. `/restart` presents a **simulated restart** button valid for 120 seconds. It has no parameters. Doing nothing cancels by expiry. There is no scheduler yet; `/digest` is on demand.

## Safety boundaries

- Strict typed JSON inventory: `mock` is the only connector; unknown keys/connectors, duplicate IDs, free-form commands and invalid IDs are rejected. Maximum 25 services, 40-character lab/service IDs.
- Approval binds owner, private chat, exact `simulate_restart` action, `lab.service`, empty parameters, creation and expiry. Immutable records and atomic SQLite transactions protect concurrent single use and audit recording.
- Requests are idempotent by Telegram update ID; replay does not extend expiry. The update cursor survives restart and resets after six days without processed updates, before Telegram may randomize update IDs after a week of inactivity. A crash after sending a reply but before saving its cursor may duplicate a reply, **not** the simulated action.
- Fixed HTTPS Telegram origin, no redirects, bounded response size/timeouts. URL/token/request bodies are never logged; error strings are deliberately generic. No LLM dependency, shell, SSH, real connector or listening port.
- SQLite is not encrypted or tamper-proof: protect its directory/backups and run unprivileged. Audit shows request/completion events, not every read or denied packet. Approval IDs are omitted from audit summaries.
- Do not treat single-use consumption as exactly-once execution of future external actions. Real adapters require target/config revision binding, adapter-side idempotency, durable execution/outbox states and reconciliation before retries. See [security design](docs/SECURITY-DESIGN.md).

## Development / next increment

`src/control.rs` contains authorization and typed commands, `inventory.rs` the mock inventory, `store.rs` the durable approval/audit ledger, and `telegram.rs` a minimal synchronous Bot API long-polling transport. This intentionally uses a small modern reqwest client instead of the obsolete telegram-bot/futures 0.1 stack; teloxide can replace the transport later without changing the approval boundary.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --offline
```

No live Telegram or user homelab integration has been tested. Choose Docker/Compose, Proxmox or Home Assistant before implementing the first real least-privilege connector. Start read-only; live writes require an explicit authorized setup and connector-specific safety review.

## Provenance

Based on upstream `swarnimarun/ferr-bot` commit `e73db16f5af3d84f447ce132cf1791742f8ba974`. Original MIT license and author attribution preserved. This is a local foundation rewrite, not a deployed service or a published PR.
