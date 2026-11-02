# Homelab control security design

Design requirements, not a certification. This foundation is for local development and mock connectors; it does not authorize connecting credentials, deploying, or operating live infrastructure.

## Trust boundaries and threats

- Telegram transports commands, not trust. Bot chats are not end-to-end encrypted. Never send credentials, private keys, recovery codes, or unrestricted logs through chat.
- Authorize the numeric sender ID **and** numeric private-chat ID before parsing commands, disclosing inventory, calling connectors, or issuing approvals. Usernames/display names are not identities. Reject group, channel, inline, edited, forwarded-origin impersonation, and unsupported update forms by default.
- Protect against stolen approval codes, duplicate updates, parallel confirmations, stale inventory, service impersonation, cross-node target confusion, hostile service output, and credential leakage. A compromised authorized Telegram account remains a residual risk; high-impact production control needs an independent authentication channel.
- Treat node names, service status, logs, connector responses and model output as data, never executable instructions or approval. A model may propose an allowlisted typed action but cannot grant authority or synthesize confirmation.
- Local configuration and the operator host are trusted in this prototype. A host compromise can change policy and read credentials; this design does not protect against that.

## Required invariants

1. **Explicit inventory:** immutable node/service identifiers, unique IDs, bounded names and per-node allowlisted capabilities. Requests resolve an exact node and service; no wildcard/bulk target, implicit default node, user-supplied URL, shell string, or arbitrary Docker API path.
2. **Least authority:** begin read-only. A consequential action requires an explicit proposal and a separate user confirmation showing exact node, service and action. Destructive/privileged operations stay unavailable until separately designed and authorized.
3. **Exact approvals:** cryptographically unpredictable opaque token, bound to actor, private chat, exact canonical action/parameters, target and policy/config revision; short TTL; cancel support; approval does not survive restart unless durably and safely implemented. Confirmation cannot replace any target or parameter. Revalidate identity, policy, inventory revision and expiry at dispatch.
4. **Single dispatch:** atomically transition pending to consumed before execution. Parallel/replayed confirmation and repeated Telegram update IDs must not execute twice. Never automatically retry an ambiguous write timeout; report unknown outcome and reconcile with a read. Persistent deployments need durable transactional state/idempotency across processes and restarts, not just a mutex.
5. **Bounded execution:** typed connector methods with input validation, timeout, output-size cap, concurrency/rate limits and explicit error states. No generic shell/SSH command passthrough and no unrestricted Docker socket exposed to the bot. Future agents should initiate authenticated outbound connections, have per-node identities and minimal permissions, and accept only bounded signed/expiry-checked action envelopes.
6. **Network containment:** future real connectors use operator-configured endpoints only; reject URL userinfo and unsupported schemes, disable redirects, validate destination and TLS identity, and enforce outbound network policy. Homelab private addresses may be intentionally allowed per endpoint, not globally. Defend DNS rebinding with destination-aware enforcement; block cloud metadata and unintended loopback/link-local targets. Never fetch URLs found in chat, logs, or model output.
7. **Audit before effect:** require successful append of intent/approval-consumption audit before dispatch; audit denial, target, actor, correlation ID, time and outcome. A failed pre-action audit blocks execution; failed post-action logging must report uncertainty rather than invite retries. Exclude bot tokens, approval tokens, secrets and raw payloads. Local files are not tamper-proof; production needs protected retention and centralized append-only storage.
8. **Secrets and presentation:** keep tokens out of source/config examples, debug output, exceptions and process arguments. Restrict credential/audit file permissions. Escape untrusted Telegram markup; bound response lengths and redact sensitive logs before sending. Never turn service content into commands or approval buttons automatically.

## Acceptance tests / release gates

Each item needs an automated test or documented integration evidence before its corresponding live feature is enabled.

| Boundary | Required negative and positive checks |
| --- | --- |
| Authorization | Allowed actor in allowed private chat succeeds; wrong actor, wrong chat, group/channel and unsupported updates do not disclose inventory or call a connector. |
| Parsing/inventory | Reject unknown node/service/action, ambiguous IDs, duplicate config IDs, extra arguments, control characters and shell/URL-like payloads; exact allowed target resolves correctly. |
| Approval | Proposal does not mutate; confirmation executes exact action once; wrong actor/chat/token, expiry, cancellation and changed target/policy cannot execute. |
| Replay/concurrency | Simultaneous confirms execute at most once; repeated update IDs do not create repeated work; restart invalidates ephemeral approvals; uncertain timeout is not retried. |
| Audit | Audit-write failure produces zero mutation calls; successful/rejected/failed/unknown operations have correlation records; serialized records contain no secrets or approval codes. |
| Connectors | Mock connector has no network/process side effects; unauthorized capabilities fail closed; deadline/output caps work; one failing node does not permit fallback to a different target. |
| Network (future) | Redirect, DNS destination change, metadata address, unauthorized port/scheme and unconfigured endpoint are blocked; configured TLS endpoint succeeds. |
| Untrusted data | Status/log strings containing prompt injection, commands, HTML/Markdown and fake confirmations remain bounded display data and cannot dispatch. |
| Deployment (future) | Missing/empty identities fail startup; credentials are permission-restricted; durable multi-worker replay protection, backup/recovery and token revocation are exercised. |

## Review scope

The document records intended invariants and future gates. Consult the implementation and test output for what is actually enforced today. Mock-only operation is not evidence that a future live connector is safe. No live-system security, deployment, secret handling or end-to-end Telegram integration has been certified here.

### Foundation review findings

The initial implementation enforces owner + same private-chat authorization, mock-only typed inventory, immutable SQLite approval fields, maximum 300-second TTL (the command uses 120 seconds), durable single consumption, request-update deduplication, and transactional audit rollback. Audit display excludes confirmation tokens. The Bot API client has a fixed HTTPS origin, disables redirects, caps responses, and uses generic errors without credential-bearing URLs. These are code-review observations backed by the local unit suite, not a live integration assessment.

Outstanding live-feature gates include policy/config-revision binding, explicit cancellation (today non-confirmation lets the request expire), denied-action audit records, credential/state-file permission enforcement, production retention/rate limits, and real connector/agent authorization and idempotency. Audit and approval tables share a protected local database; the underlying database contains capability-bearing approval IDs even though audit display omits them. An outbound response failure after a committed mock action may lose the success reply; repeated confirmation remains rejected rather than rerunning the action. Review these boundaries again before adding any external side effect.

Reviewer verification (2026-11-01): independently ran `cargo test --offline --lib` after composite `lab.service` target binding was added: **18 passed, 0 failed**. Coverage includes different-lab invalidation, invalid actor/chat/context, malformed and expired approvals, replay, concurrent consumption, durable reopen, immutability, audit rollback/token omission, strict inventory and token URL-injection rejection. No live Telegram session or homelab endpoint was used. No blocker was identified for the explicitly mock-only foundation; this is not approval to deploy or add live control without the gates above.
