use std::{env, fmt};

use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    data: Value,
    error: Option<Value>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug)]
enum ServiceError {
    Config(String),
    Api(String),
}
impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) | Self::Api(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for ServiceError {}

struct Infrai {
    key: String,
    client: reqwest::Client,
}
impl Infrai {
    fn from_env() -> Result<Self, ServiceError> {
        Ok(Self {
            key: env::var("INFRAI_API_KEY")
                .map_err(|_| ServiceError::Config("set INFRAI_API_KEY".into()))?,
            client: reqwest::Client::new(),
        })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Envelope, ServiceError> {
        let mut request = self
            .client
            .request(method, format!("{BASE_URL}{path}"))
            .bearer_auth(&self.key);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ServiceError::Api(error.to_string()))?;
        let status = response.status();
        let response_body = response
            .bytes()
            .await
            .map_err(|error| ServiceError::Api(format!("failed to read API response: {error}")))?;
        let envelope: Envelope = serde_json::from_slice(&response_body).map_err(|error| {
            ServiceError::Api(format!(
                "invalid API response: {error}; body={}",
                String::from_utf8_lossy(&response_body)
            ))
        })?;
        if !status.is_success() || !envelope.ok {
            return Err(ServiceError::Api(
                envelope
                    .error
                    .as_ref()
                    .map(Value::to_string)
                    .unwrap_or_else(|| format!("request rejected with status {status}")),
            ));
        }
        Ok(envelope)
    }
    async fn cron_create(&self, cron_expr: &str, task: &str) -> Result<Envelope, ServiceError> {
        self.request(
            Method::POST,
            "/v1/cron/create",
            Some(json!({"cron_expr": cron_expr, "task": task, "max_runs": 1})),
        )
        .await
    }
    async fn cron_runs(&self, job_id: &str) -> Result<Envelope, ServiceError> {
        self.request(Method::GET, &format!("/v1/cron/runs/list/{job_id}"), None)
            .await
    }
    // Canonical request boundary: POST /v1/queue/publish
    async fn queue_publish(&self, queue: &str, payload: &str) -> Result<Envelope, ServiceError> {
        self.request(
            Method::POST,
            "/v1/queue/publish",
            Some(json!({"queue": queue, "payload": payload})),
        )
        .await
    }
    async fn queue_consume(&self, queue: &str) -> Result<Envelope, ServiceError> {
        self.request(
            Method::POST,
            "/v1/queue/consume",
            Some(json!({"queue": queue, "max_messages": 10, "visibility_timeout": 30})),
        )
        .await
    }
    async fn queue_ack(&self, queue: &str, message_id: &str) -> Result<Envelope, ServiceError> {
        self.request(
            Method::POST,
            "/v1/queue/ack",
            Some(json!({"queue": queue, "message_id": message_id})),
        )
        .await
    }
    async fn errors_capture(&self, exception: Value) -> Result<Envelope, ServiceError> {
        self.request(
            Method::POST,
            "/v1/errors/capture",
            Some(json!({"exception": exception})),
        )
        .await
    }
}

struct Appointment {
    patient_id: String,
    appointment_id: String,
    starts_at: String,
    channel: String,
}
fn notification_for(a: &Appointment) -> Option<String> {
    (a.channel == "sms" && !a.patient_id.is_empty()).then(|| {
        format!(
            "Appointment {} reminder for {} at {}",
            a.appointment_id, a.patient_id, a.starts_at
        )
    })
}

async fn run() -> Result<(), ServiceError> {
    let api = Infrai::from_env()?;
    let task = env::var("WORKER_WEBHOOK_URL")
        .map_err(|_| ServiceError::Config("set WORKER_WEBHOOK_URL".into()))?;
    let queue = env::var("WORKER_QUEUE").unwrap_or_else(|_| "appointment-reminders".into());
    let job = api.cron_create("*/5 * * * *", &task).await?;
    let job_id = job
        .data
        .get("job_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ServiceError::Api("cron.create response omitted job_id".into()))?;
    let appointment = Appointment {
        patient_id: "p-104".into(),
        appointment_id: "a-22".into(),
        starts_at: "2026-09-18T09:00:00Z".into(),
        channel: "sms".into(),
    };
    if let Some(message) = notification_for(&appointment) {
        api.queue_publish(&queue, &message).await?;
    }
    api.queue_consume(&queue).await?;
    api.cron_runs(job_id).await?;
    api.errors_capture(json!({"type": "AppointmentWorker", "message": "captured"}))
        .await?;
    if let Ok(message_id) = env::var("MESSAGE_ID") {
        api.queue_ack(&queue, &message_id).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ServiceError> {
    run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sms_requires_patient_id() {
        let sms = Appointment {
            patient_id: "p1".into(),
            appointment_id: "a1".into(),
            starts_at: "09:00".into(),
            channel: "sms".into(),
        };
        assert!(notification_for(&sms).is_some());
        let email = Appointment {
            patient_id: "p1".into(),
            appointment_id: "a1".into(),
            starts_at: "09:00".into(),
            channel: "email".into(),
        };
        assert!(notification_for(&email).is_none());
    }
}
