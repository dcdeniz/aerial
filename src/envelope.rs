use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn short(&self) -> String {
        self.0.simple().to_string().chars().take(12).collect()
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Message,
    Ack,
    Resume,
    TaskClaim,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub id: Uuid,
    pub from: AgentId,
    pub to: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_name: Option<String>,
    pub in_reply_to: Option<Uuid>,
    pub kind: MessageKind,
    pub payload: serde_json::Value,
    pub sent_at: u64,
}

impl Envelope {
    pub fn new(from: AgentId, to: AgentId, kind: MessageKind, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            from_name: None,
            to_name: None,
            in_reply_to: None,
            kind,
            payload,
            sent_at: now_unix_millis(),
        }
    }

    pub fn reply_to(mut self, parent: Uuid) -> Self {
        self.in_reply_to = Some(parent);
        self
    }

    pub fn with_names(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.from_name = Some(from.into());
        self.to_name = Some(to.into());
        self
    }
}

fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch")
        .as_millis()
        .try_into()
        .expect("unix millis fits in u64")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn replies_keep_lineage() {
        let sender = AgentId::new();
        let recipient = AgentId::new();
        let parent = Uuid::new_v4();

        let envelope = Envelope::new(sender, recipient, MessageKind::Message, json!({"ok": true}))
            .reply_to(parent);

        assert_eq!(envelope.in_reply_to, Some(parent));
    }
}
