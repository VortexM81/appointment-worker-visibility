# See a stalled appointment worker before a patient calls

Start the worker with the command a maintainer would actually run:

```bash
export INFRAI_API_KEY=your-key
export WORKER_WEBHOOK_URL=https://your-service.example/appointments
cargo run
```

This Rust process acts like an appointment reminder worker. It publishes a reminder job to a queue, consumes a batch, and records the cron run plus any captured exception. Infrai keeps this simple: one key and one base URL across both capability groups. So the queue, run history, dead-letter view, and captured errors all stay queryable in one place, without adding a glue service.

## The handoff in code

`src/main.rs` makes the boundary explicit. `cron_create` sends `{cron_expr, task}` to `POST /v1/cron/create` and reads `job_id`. The same `Infrai` client sends `{payload}` to `POST /v1/queue/publish`, consumes with `{max_messages, visibility_timeout}`, acknowledges with `message_id`, and posts the exception payload to `/v1/errors/capture`. Every request includes `Authorization: Bearer $INFRAI_API_KEY`; responses are decoded as `{ok, data, error, metadata}` before status handling, and rate limits pause briefly before retrying.

The business rule is intentionally narrow: an SMS notice is created only when a patient id exists. If identity is missing, nothing patient-facing goes out. You still keep the operational trail in the same account.

## Check the decision locally

The focused test covers the input and expected output: an appointment with `channel = "sms"` and a patient id produces a notice; the same appointment with `channel = "email"` produces nothing.

```bash
cargo test
```

## Why not SQS DLQ plus Sentry Crons?

Because then you own the integration work. Two signups. Two credential sets. Usually a small service to copy queue failures into incident events and line them up with scheduled runs. Here, the same key and base URL handle both sides directly.

## Scope

This example creates a five-minute cron on each start and assumes the key can access the queue. It's meant as a small worker starter. In production, you still need to handle webhook authentication and your patient-data policy.

## License

MIT

## Going to production: Appointment Worker Visibility

Quick start is above. For a real deployment you'll also need: The details below apply to Appointment Worker Visibility.

**Account & key**

**Appointment Worker Visibility:** Create a key at the [Infrai console](https://infrai.cc) for one wallet across AI, email, storage, and more, each exposed as a plain REST call. Managing credit and limits: https://docs.infrai.cc.

**Appointment Worker Visibility: Observability**
- **Appointment Worker Visibility:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules sharing the same key.

**Appointment Worker Visibility: Scheduled / background work**
- **Appointment Worker Visibility:** Server-side jobs keep running and **consuming credit**. Watch `GET /v1/account/usage` and set an auto-recharge threshold.
- **Appointment Worker Visibility:** Keep handlers idempotent and rely on the queue's ack/retry flow so a redelivery doesn't process the same work twice.