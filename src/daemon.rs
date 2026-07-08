use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use serde_json::json;
use thiserror::Error;

use crate::envelope::{AgentId, Envelope, MessageKind};
use crate::mailbox::{Mailbox, MailboxError};
use crate::protocol::{DaemonRequest, DaemonResponse};
use crate::registry::Registry;
use crate::transcript::{Transcript, TranscriptError, TranscriptMessage};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("mailbox error: {0}")]
    Mailbox(#[from] MailboxError),
    #[error("transcript error: {0}")]
    Transcript(#[from] TranscriptError),
    #[error("protocol decode error: {0}")]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct Daemon {
    data_dir: PathBuf,
    registry: Registry,
    known_agents: HashMap<String, AgentId>,
}

impl Daemon {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, DaemonError> {
        let data_dir = data_dir.into();
        fs::create_dir_all(mailboxes_dir(&data_dir)).map_err(|source| DaemonError::Io {
            path: data_dir.clone(),
            source,
        })?;

        Ok(Self {
            data_dir,
            registry: Registry::new(),
            known_agents: HashMap::new(),
        })
    }

    pub fn socket_path(&self) -> PathBuf {
        self.data_dir.join("aerial.sock")
    }

    pub fn serve(mut self) -> Result<(), DaemonError> {
        let socket_path = self.socket_path();
        if socket_path.exists() {
            fs::remove_file(&socket_path).map_err(|source| DaemonError::Io {
                path: socket_path.clone(),
                source,
            })?;
        }

        let listener = UnixListener::bind(&socket_path).map_err(|source| DaemonError::Io {
            path: socket_path.clone(),
            source,
        })?;

        for stream in listener.incoming() {
            let mut stream = stream.map_err(|source| DaemonError::Io {
                path: socket_path.clone(),
                source,
            })?;
            let response = match read_request(&stream).and_then(|request| self.handle(request)) {
                Ok(response) => response,
                Err(error) => DaemonResponse::Error {
                    message: error.to_string(),
                },
            };
            write_response(&mut stream, &response).map_err(|source| DaemonError::Io {
                path: socket_path.clone(),
                source,
            })?;
        }

        Ok(())
    }

    pub fn handle(&mut self, request: DaemonRequest) -> Result<DaemonResponse, DaemonError> {
        match request {
            DaemonRequest::Register { name } => {
                let registered = self.registry.register(name.clone());
                self.known_agents
                    .insert(name.clone(), registered.id.clone());
                Ok(DaemonResponse::Registered {
                    name,
                    id: registered.id,
                })
            }
            DaemonRequest::Send {
                from,
                to,
                body,
                in_reply_to,
            } => {
                let from_id = self.agent_id(&from);
                let to_id = self.agent_id(&to);
                let mut envelope = Envelope::new(
                    from_id,
                    to_id,
                    MessageKind::Message,
                    json!({ "body": body }),
                );
                envelope.in_reply_to = in_reply_to;

                self.mailbox(&to)?.enqueue(&envelope)?;
                self.transcript()?.append(&TranscriptMessage {
                    envelope: envelope.clone(),
                    from_name: from,
                    to_name: to,
                })?;
                Ok(DaemonResponse::Sent { envelope })
            }
            DaemonRequest::Pending { agent } => {
                let envelopes = self.mailbox(&agent)?.pending()?;
                Ok(DaemonResponse::Pending { envelopes })
            }
            DaemonRequest::Ack { agent, id } => {
                self.mailbox(&agent)?.ack(id)?;
                Ok(DaemonResponse::Acked { id })
            }
            DaemonRequest::History { limit } => {
                let messages = self.transcript()?.tail(limit)?;
                Ok(DaemonResponse::History { messages })
            }
        }
    }

    fn agent_id(&mut self, name: &str) -> AgentId {
        if let Some(agent) = self.registry.resolve(name) {
            return agent.id.clone();
        }
        if let Some(id) = self.known_agents.get(name) {
            return id.clone();
        }

        let id = AgentId::new();
        self.known_agents.insert(name.to_owned(), id.clone());
        id
    }

    fn mailbox(&self, agent: &str) -> Result<Mailbox, DaemonError> {
        Mailbox::open(mailboxes_dir(&self.data_dir).join(format!("{agent}.jsonl")))
            .map_err(Into::into)
    }

    fn transcript(&self) -> Result<Transcript, DaemonError> {
        Transcript::open(self.data_dir.join("history.jsonl")).map_err(Into::into)
    }
}

pub fn request(socket_path: &Path, request: &DaemonRequest) -> Result<DaemonResponse, DaemonError> {
    let mut stream = UnixStream::connect(socket_path).map_err(|source| DaemonError::Io {
        path: socket_path.to_path_buf(),
        source,
    })?;
    serde_json::to_writer(&mut stream, request)?;
    stream.write_all(b"\n").map_err(|source| DaemonError::Io {
        path: socket_path.to_path_buf(),
        source,
    })?;
    read_response(&stream)
}

fn mailboxes_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("mailboxes")
}

fn read_request(stream: &UnixStream) -> Result<DaemonRequest, DaemonError> {
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|source| DaemonError::Io {
            path: PathBuf::from("<socket>"),
            source,
        })?;
    serde_json::from_str(&line).map_err(Into::into)
}

fn read_response(stream: &UnixStream) -> Result<DaemonResponse, DaemonError> {
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|source| DaemonError::Io {
            path: PathBuf::from("<socket>"),
            source,
        })?;
    serde_json::from_str(&line).map_err(Into::into)
}

fn write_response(
    stream: &mut UnixStream,
    response: &DaemonResponse,
) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut *stream, response)?;
    stream.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_sends_to_named_mailbox_and_acks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut daemon = Daemon::new(dir.path()).expect("daemon");

        let sent = daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "status?".to_owned(),
                in_reply_to: None,
            })
            .expect("send");

        let envelope = match sent {
            DaemonResponse::Sent { envelope } => envelope,
            other => panic!("unexpected response: {other:?}"),
        };

        assert!(matches!(
            daemon
                .handle(DaemonRequest::Pending {
                    agent: "researcher".to_owned()
                })
                .expect("pending"),
            DaemonResponse::Pending { ref envelopes } if envelopes == &vec![envelope.clone()]
        ));

        daemon
            .handle(DaemonRequest::Ack {
                agent: "researcher".to_owned(),
                id: envelope.id,
            })
            .expect("ack");

        assert!(matches!(
            daemon
                .handle(DaemonRequest::Pending {
                    agent: "researcher".to_owned()
                })
                .expect("pending after ack"),
            DaemonResponse::Pending { ref envelopes } if envelopes.is_empty()
        ));
    }

    #[test]
    fn daemon_records_send_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut daemon = Daemon::new(dir.path()).expect("daemon");

        daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "please inspect the docs".to_owned(),
                in_reply_to: None,
            })
            .expect("send");

        let history = daemon
            .handle(DaemonRequest::History { limit: Some(1) })
            .expect("history");

        assert!(matches!(
            history,
            DaemonResponse::History { ref messages }
                if messages.len() == 1
                    && messages[0].from_name == "engineer"
                    && messages[0].to_name == "researcher"
                    && messages[0].envelope.payload["body"] == "please inspect the docs"
        ));
    }
}
