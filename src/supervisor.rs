use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use thiserror::Error;
use uuid::Uuid;

use crate::daemon::{self, DaemonError};
use crate::{DaemonRequest, DaemonResponse, Envelope, TranscriptMessage};

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("agent command is empty")]
    EmptyCommand,
    #[error("daemon error: {0}")]
    Daemon(#[from] DaemonError),
    #[error("message {id} is not pending for agent {agent}")]
    MissingMessage { agent: String, id: Uuid },
    #[error("unexpected daemon response: {0:?}")]
    UnexpectedResponse(DaemonResponse),
    #[error("failed to encode envelope: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("failed to run agent command {program}: {source}")]
    CommandIo {
        program: String,
        source: std::io::Error,
    },
}

#[derive(Clone, Debug)]
pub struct AgentMessage {
    pub agent: String,
    pub envelope: Envelope,
    pub history: Vec<TranscriptMessage>,
}

impl AgentMessage {
    pub fn id(&self) -> Uuid {
        self.envelope.id
    }

    pub fn body(&self) -> &str {
        self.envelope
            .payload
            .get("body")
            .and_then(|value| value.as_str())
            .unwrap_or("")
    }
}

#[derive(Clone, Debug)]
pub struct SupervisorOptions {
    pub socket: PathBuf,
    pub agent: String,
    pub history_limit: Option<usize>,
}

pub fn load_message(
    socket: &Path,
    agent: &str,
    id: Uuid,
    history_limit: Option<usize>,
) -> Result<AgentMessage, SupervisorError> {
    let pending = daemon::request(
        socket,
        &DaemonRequest::Pending {
            agent: agent.to_owned(),
        },
    )?;
    let envelope = match pending {
        DaemonResponse::Pending { envelopes } => envelopes
            .into_iter()
            .find(|envelope| envelope.id == id)
            .ok_or_else(|| SupervisorError::MissingMessage {
                agent: agent.to_owned(),
                id,
            })?,
        other => return Err(SupervisorError::UnexpectedResponse(other)),
    };

    let history = match daemon::request(
        socket,
        &DaemonRequest::History {
            limit: history_limit,
        },
    )? {
        DaemonResponse::History { messages } => messages,
        other => return Err(SupervisorError::UnexpectedResponse(other)),
    };

    Ok(AgentMessage {
        agent: agent.to_owned(),
        envelope,
        history,
    })
}

pub fn run_exec_agent(
    options: &SupervisorOptions,
    id: Uuid,
    command: &[String],
) -> Result<ExitStatus, SupervisorError> {
    if command.is_empty() {
        return Err(SupervisorError::EmptyCommand);
    }

    let message = load_message(&options.socket, &options.agent, id, options.history_limit)?;
    let status = spawn_agent_command(&options.socket, &message, command)?;
    if status.success() {
        ack(&options.socket, &options.agent, id)?;
    }
    Ok(status)
}

pub fn codex_command(
    socket: &Path,
    agent: &str,
    id: Uuid,
    cd: &Path,
    approval: &str,
    history_limit: Option<usize>,
) -> Result<Vec<String>, SupervisorError> {
    let message = load_message(socket, agent, id, history_limit)?;
    Ok(vec![
        "codex".to_owned(),
        "--ask-for-approval".to_owned(),
        approval.to_owned(),
        "exec".to_owned(),
        "--cd".to_owned(),
        cd.display().to_string(),
        codex_prompt(&message),
    ])
}

pub fn codex_prompt(message: &AgentMessage) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are an Aerial-managed coding agent.\n\n");
    prompt.push_str("Aerial message context:\n");
    prompt.push_str(&format!("- agent: {}\n", message.agent));
    prompt.push_str(&format!("- envelope_id: {}\n", message.id()));
    prompt.push_str(&format!("- body: {}\n\n", message.body()));
    if !message.history.is_empty() {
        prompt.push_str("Recent Aerial history:\n");
        for item in &message.history {
            prompt.push_str("- ");
            prompt.push_str(&item.render_summary());
            prompt.push('\n');
        }
        prompt.push('\n');
    }
    prompt.push_str("Do the requested work in the current repository. ");
    prompt.push_str("Do not acknowledge the Aerial message yourself; ");
    prompt.push_str("the Aerial supervisor will ack it only if this command exits successfully.");
    prompt
}

fn spawn_agent_command(
    socket: &Path,
    message: &AgentMessage,
    command: &[String],
) -> Result<ExitStatus, SupervisorError> {
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    child
        .env("AERIAL_AGENT", &message.agent)
        .env("AERIAL_MESSAGE_ID", message.id().to_string())
        .env("AERIAL_MESSAGE_BODY", message.body())
        .env("AERIAL_SOCKET", socket)
        .env(
            "AERIAL_ENVELOPE_JSON",
            serde_json::to_string(&message.envelope)?,
        );

    child.status().map_err(|source| SupervisorError::CommandIo {
        program: command[0].clone(),
        source,
    })
}

fn ack(socket: &Path, agent: &str, id: Uuid) -> Result<(), SupervisorError> {
    match daemon::request(
        socket,
        &DaemonRequest::Ack {
            agent: agent.to_owned(),
            id,
        },
    )? {
        DaemonResponse::Acked { .. } => Ok(()),
        other => Err(SupervisorError::UnexpectedResponse(other)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::envelope::{AgentId, MessageKind};

    use super::*;

    #[test]
    fn codex_prompt_includes_message_and_history() {
        let envelope = Envelope::new(
            AgentId::new(),
            AgentId::new(),
            MessageKind::Message,
            json!({ "body": "Update the launch page." }),
        );
        let message = AgentMessage {
            agent: "agent2".to_owned(),
            envelope,
            history: Vec::new(),
        };

        let prompt = codex_prompt(&message);

        assert!(prompt.contains("agent: agent2"));
        assert!(prompt.contains("Update the launch page."));
        assert!(prompt.contains("will ack it only if this command exits successfully"));
    }
}
