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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DaemonResponse {
    Registered { name: String, id: AgentId },
    Sent { envelope: Envelope },
    Pending { envelopes: Vec<Envelope> },
    Acked { id: Uuid },
    History { messages: Vec<TranscriptMessage> },
    Error { message: String },
}
