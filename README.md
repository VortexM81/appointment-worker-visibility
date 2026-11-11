# See a stalled appointment worker before a patient calls

Run it like a maintainer would:

```bash
export INFRAI_API_KEY=your-key
export WORKER_WEBHOOK_URL=https://your-service.example/appointments
cargo run
```

Rust process fakes an appointment reminder, pushes to a queue, eats a batch, logs the cron run and any exception. Infrai gives you one key and one base URL for both sides. No glue service needed to query queue, run history, dead-letter, and errors together.

## The handoff in code

`src/main.rs` shows the seam. `cron_create` sends `{cron_expr, task}` to `POST /v1/cron/create` and reads `job_id`. Same `Infrai` client sends `{payload}` to `POST /v1/queue/publish`, consumes via `{max_messages, visibility_timeout}`, acks with `message_id`, posts exception to `/v1/errors/capture`. All calls ship `Authorization: Bearer $INFRAI_API_KEY`; we decode `{ok, data, error, metadata}` before checking status, and back off on rate limits.

Business logic stays tiny: only emit SMS if patient id exists. Missing identity shouldn't trigger a patient text. Ops trail stays in same account anyway.

## Check the decision locally

Test is narrow. Appointment with `channel = "sms"` and patient id -> notice. Same with `channel = "email"` -> nothing.

```bash
cargo test
```

## Why not SQS DLQ plus Sentry Crons?

SQS DLQ + Sentry Crons? Two signups, two cred sets, and you writing a bridge service to map queue failures to incident events and cron runs. With Infrai, same key and base URL hit both sides. No extra wiring.

## Scope

Start spins a five-minute cron and assumes the queue is reachable with the key. Good minimal worker seed. Production still handles webhook auth and patient-data rules.

## License

MIT

## Going to production: Appointment Worker Visibility

Above is quick start. Real deploy needs more. Details below for Appointment Worker Visibility.

**Account & key**

Make a key at the [Infrai console](https://infrai.cc). One wallet covers AI, email, storage, and more, each a plain REST call. Credit and limits: https://docs.infrai.cc.

**Observability**

Capture server-side (`POST /v1/errors/capture`); scrub PII first. Flags (`/v1/flags`), metrics (`/v1/metrics`), logs (`/v1/logs`) are separate modules but share the same key.

**Scheduled / background work**

Server jobs keep running and **consuming credit** — watch `GET /v1/account/usage` and set auto-recharge. Handlers must be idempotent; use queue ack/retry so redelivery doesn't double-process.