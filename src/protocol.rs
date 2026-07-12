use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::envelope::{AgentId, Envelope};
use crate::transcript::TranscriptMessage;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DaemonRequest {
    Register {
        name: String,
    },
    Send {
        from: String,
        to: String,
        body: String,
        in_reply_to: Option<Uuid>,
        #[serde(default)]
        create: bool,
    },
    Pending {
        agent: String,
    },
    Ack {
        agent: String,
        id: Uuid,
    },
    History {
        limit: Option<usize>,
    },
    Agents,
    Watch {
        agent: String,
    },
}

/// A wake notification streamed over a `Watch` connection. The mailbox remains
/// the source of truth; an event only signals that a pending envelope exists,
/// so a lost or duplicated event never loses a message.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WatchEvent {
    Message { agent: String, id: Uuid },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DaemonResponse {
    Registered { name: String, id: AgentId },
    Sent { envelope: Envelope },
    Pending { envelopes: Vec<Envelope> },
    Acked { id: Uuid },
    History { messages: Vec<TranscriptMessage> },
    Agents { agents: Vec<AgentStatus> },
    Error { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentStatus {
    pub name: String,
    pub id: AgentId,
    pub pending: usize,
    pub last_seen: u64,
}
