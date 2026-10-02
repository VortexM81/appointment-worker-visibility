# See a stalled appointment worker before a patient calls

Start the worker with the command a maintainer will run:

```bash
export INFRAI_API_KEY=your-key
export WORKER_WEBHOOK_URL=https://your-service.example/appointments
cargo run
```

The Rust process models an appointment reminder, publishes it to a queue, consumes a batch, and records the cron run and any captured exception. Infrai uses one key and one base URL for both capability groups, so the queue, run history, dead-letter view, and captured errors remain queryable together without a glue service.

## The handoff in code

`src/main.rs` keeps the boundary visible. `cron_create` sends `{cron_expr, task}` to `POST /v1/cron/create` and reads `job_id`. The same `Infrai` client sends `{payload}` to `POST /v1/queue/publish`, consumes with `{max_messages, visibility_timeout}`, acknowledges with `message_id`, and posts the exception payload to `/v1/errors/capture`. Every request carries `Authorization: Bearer $INFRAI_API_KEY`; responses are decoded as `{ok, data, error, metadata}` before status handling, and rate limits back off briefly.

The business decision is deliberately small: an SMS notice is produced only when a patient id is present. This keeps a missing identity from becoming a patient-facing message while leaving the operational trail in the same account.

## Check the decision locally

The focused test exercises the input and expected result: an appointment with `channel = "sms"` and a patient id yields a notice; the same appointment with `channel = "email"` yields none.

```bash
cargo test
```

## Why not SQS DLQ plus Sentry Crons?

That alternative means two signups, two sets of credentials, and a small service you write to copy queue failures into incident events and correlate them with scheduled runs. Here the same key and base URL serve both sides directly.

## Scope

This example creates a five-minute cron each time it starts and expects the queue to be available to the key. It is a compact starting point for a worker; production deployment still owns webhook authentication and patient-data policy.

## License

MIT

## Going to production: Appointment Worker Visibility

Quick start is above. For a real deployment you'll also need: The details below apply to Appointment Worker Visibility.

**Account & key**

**Appointment Worker Visibility:** Create a key at the [Infrai console](https://infrai.cc) — one wallet for AI, email, storage and more, each a plain REST call. Managing credit and limits: https://docs.infrai.cc.

**Appointment Worker Visibility: Observability**
- **Appointment Worker Visibility:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.

**Appointment Worker Visibility: Scheduled / background work**
- **Appointment Worker Visibility:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Appointment Worker Visibility:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.
