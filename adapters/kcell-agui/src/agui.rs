//! Minimal AG-UI-compatible run input + event builders.

use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentInput {
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub content: Value,
}

pub struct RunIds {
    pub thread_id: String,
    pub run_id: String,
    pub message_id: String,
}

pub fn resolve_ids(input: &RunAgentInput) -> RunIds {
    let stamp = now_stamp();
    RunIds {
        thread_id: input
            .thread_id
            .clone()
            .unwrap_or_else(|| format!("thread-{stamp}")),
        run_id: input
            .run_id
            .clone()
            .unwrap_or_else(|| format!("run-{stamp}")),
        message_id: format!("msg-{stamp}"),
    }
}

/// Last user message → envelope payload.
pub fn payload_from_messages(messages: &[Message]) -> Result<Value, String> {
    let user = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .ok_or_else(|| "RunAgentInput requires a user message".to_string())?;
    match &user.content {
        Value::String(s) => {
            // Try JSON object/array first; else wrap as { "text": ... }
            if let Ok(v) = serde_json::from_str::<Value>(s) {
                if v.is_object() || v.is_array() {
                    return Ok(v);
                }
            }
            Ok(json!({ "text": s }))
        }
        Value::Null => Ok(json!({})),
        other => Ok(other.clone()),
    }
}

pub fn event_run_started(ids: &RunIds) -> Value {
    json!({
        "type": "RUN_STARTED",
        "threadId": ids.thread_id,
        "runId": ids.run_id,
    })
}

pub fn event_text_start(ids: &RunIds) -> Value {
    json!({
        "type": "TEXT_MESSAGE_START",
        "messageId": ids.message_id,
        "role": "assistant",
    })
}

pub fn event_text_content(ids: &RunIds, delta: &str) -> Value {
    json!({
        "type": "TEXT_MESSAGE_CONTENT",
        "messageId": ids.message_id,
        "delta": delta,
    })
}

pub fn event_text_end(ids: &RunIds) -> Value {
    json!({
        "type": "TEXT_MESSAGE_END",
        "messageId": ids.message_id,
    })
}

pub fn event_run_finished(ids: &RunIds) -> Value {
    json!({
        "type": "RUN_FINISHED",
        "threadId": ids.thread_id,
        "runId": ids.run_id,
    })
}

pub fn event_run_error(ids: &RunIds, message: &str) -> Value {
    json!({
        "type": "RUN_ERROR",
        "threadId": ids.thread_id,
        "runId": ids.run_id,
        "message": message,
    })
}

fn now_stamp() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_from_user_text() {
        let msgs = vec![
            Message {
                role: "system".into(),
                content: json!("ignore"),
            },
            Message {
                role: "user".into(),
                content: json!("hello"),
            },
        ];
        assert_eq!(payload_from_messages(&msgs).unwrap(), json!({"text":"hello"}));
    }

    #[test]
    fn payload_from_user_json_string() {
        let msgs = vec![Message {
            role: "user".into(),
            content: json!("{\"ping\":true}"),
        }];
        assert_eq!(payload_from_messages(&msgs).unwrap(), json!({"ping":true}));
    }

    #[test]
    fn event_shapes() {
        let ids = RunIds {
            thread_id: "t1".into(),
            run_id: "r1".into(),
            message_id: "m1".into(),
        };
        assert_eq!(event_run_started(&ids)["type"], "RUN_STARTED");
        assert_eq!(event_text_start(&ids)["role"], "assistant");
        assert_eq!(event_text_content(&ids, "hi")["delta"], "hi");
        assert_eq!(event_run_finished(&ids)["runId"], "r1");
        assert_eq!(event_run_error(&ids, "x")["message"], "x");
    }
}
