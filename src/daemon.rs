use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::UNIX_EPOCH;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::envelope::{AgentId, Envelope, MessageKind};
use crate::mailbox::{Mailbox, MailboxError};
use crate::protocol::{AgentStatus, DaemonRequest, DaemonResponse, WatchEvent};
use crate::registry::Registry;
use crate::transcript::{Transcript, TranscriptError, TranscriptMessage};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "cannot connect to the Aerial daemon at {socket}: {source}. Start it with `aerial up --data-dir {data_dir}`"
    )]
    Connect {
        socket: PathBuf,
        data_dir: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "unknown recipient `{0}`; register it with `aerial join {0}` or resend with `--create`"
    )]
    UnknownRecipient(String),
    #[error("mailbox error: {0}")]
    Mailbox(#[from] MailboxError),
    #[error("transcript error: {0}")]
    Transcript(#[from] TranscriptError),
    #[error("protocol decode error: {0}")]
    Decode(#[from] serde_json::Error),
}

/// State shared across connection threads, guarded by a single lock.
#[derive(Debug, Default)]
struct DaemonState {
    registry: Registry,
    known_agents: HashMap<String, AgentId>,
}

impl DaemonState {
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
}

#[derive(Debug)]
pub struct Daemon {
    data_dir: PathBuf,
    state: Mutex<DaemonState>,
    /// Per-agent list of live `watch` subscribers to notify on new mail.
    watchers: Mutex<HashMap<String, Vec<Sender<WatchEvent>>>>,
}

impl Daemon {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, DaemonError> {
        let data_dir = data_dir.into();
        fs::create_dir_all(mailboxes_dir(&data_dir)).map_err(|source| DaemonError::Io {
            path: data_dir.clone(),
            source,
        })?;

        let state = restore_state(&data_dir)?;

        Ok(Self {
            data_dir,
            state: Mutex::new(state),
            watchers: Mutex::new(HashMap::new()),
        })
    }

    pub fn socket_path(&self) -> PathBuf {
        self.data_dir.join("aerial.sock")
    }

    pub fn serve(self) -> Result<(), DaemonError> {
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

        let daemon = Arc::new(self);
        for stream in listener.incoming() {
            let stream = stream.map_err(|source| DaemonError::Io {
                path: socket_path.clone(),
                source,
            })?;
            let daemon = Arc::clone(&daemon);
            thread::spawn(move || {
                if let Err(error) = daemon.serve_connection(stream) {
                    eprintln!("aerial: connection error: {error}");
                }
            });
        }

        Ok(())
    }

    /// Handle a single accepted connection. Most requests are one-shot
    /// request/response; `Watch` upgrades the connection to a long-lived
    /// event stream instead.
    fn serve_connection(&self, mut stream: UnixStream) -> Result<(), DaemonError> {
        let request = match read_request(&stream) {
            Ok(request) => request,
            Err(error) => {
                let _ = write_response(
                    &mut stream,
                    &DaemonResponse::Error {
                        message: error.to_string(),
                    },
                );
                return Ok(());
            }
        };

        if let DaemonRequest::Watch { agent } = request {
            return self.serve_watch(stream, agent);
        }

        let response = self
            .handle(request)
            .unwrap_or_else(|error| DaemonResponse::Error {
                message: error.to_string(),
            });
        write_response(&mut stream, &response).map_err(|source| DaemonError::Io {
            path: PathBuf::from("<socket>"),
            source,
        })
    }

    /// Stream wake notifications for `agent` until the client disconnects.
    /// Any envelopes already pending on subscribe are announced first, so a
    /// watcher that attaches late still learns about waiting mail.
    fn serve_watch(&self, mut stream: UnixStream, agent: String) -> Result<(), DaemonError> {
        let (tx, rx) = mpsc::channel::<WatchEvent>();

        // Register before scanning pending mail so a message that arrives
        // during the scan is never missed (a duplicate event is harmless —
        // the mailbox is the source of truth).
        self.watchers
            .lock()
            .expect("watchers lock")
            .entry(agent.clone())
            .or_default()
            .push(tx.clone());

        for envelope in self.mailbox(&agent)?.pending()? {
            let _ = tx.send(WatchEvent::Message {
                agent: agent.clone(),
                id: envelope.id,
            });
        }
        drop(tx);

        for event in rx {
            if write_event(&mut stream, &event).is_err() {
                break;
            }
        }
        Ok(())
    }

    pub fn handle(&self, request: DaemonRequest) -> Result<DaemonResponse, DaemonError> {
        match request {
            DaemonRequest::Register { name } => {
                self.mailbox(&name)?;
                let mut state = self.state.lock().expect("state lock");
                let registered = state.registry.register(name.clone());
                state
                    .known_agents
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
                create,
            } => {
                let (from_id, to_id) = {
                    let mut state = self.state.lock().expect("state lock");
                    let to_id = match state.registry.resolve(&to) {
                        Some(agent) => agent.id.clone(),
                        None if create => {
                            let registered = state.registry.register(to.clone());
                            state.known_agents.insert(to.clone(), registered.id.clone());
                            registered.id
                        }
                        None => return Err(DaemonError::UnknownRecipient(to)),
                    };
                    (state.agent_id(&from), to_id)
                };
                let mut envelope = Envelope::new(
                    from_id,
                    to_id,
                    MessageKind::Message,
                    json!({ "body": body }),
                )
                .with_names(from.clone(), to.clone());
                envelope.in_reply_to = in_reply_to;

                self.mailbox(&to)?.enqueue(&envelope)?;
                self.transcript()?.append(&TranscriptMessage {
                    envelope: envelope.clone(),
                    from_name: from,
                    to_name: to.clone(),
                })?;
                self.notify_watchers(&to, envelope.id);
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
            DaemonRequest::Agents => {
                let registered = self.state.lock().expect("state lock").registry.agents();
                let mut agents = Vec::with_capacity(registered.len());
                for agent in registered {
                    agents.push(AgentStatus {
                        pending: self.mailbox(&agent.name)?.pending()?.len(),
                        name: agent.name,
                        id: agent.id,
                        last_seen: agent.connected_at,
                    });
                }
                Ok(DaemonResponse::Agents { agents })
            }
            DaemonRequest::Watch { .. } => Ok(DaemonResponse::Error {
                message: "watch is a streaming request; connect and read events".to_owned(),
            }),
        }
    }

    /// Notify every live watcher of `agent` that a new envelope is pending,
    /// pruning any whose client has gone away.
    fn notify_watchers(&self, agent: &str, id: Uuid) {
        let mut watchers = self.watchers.lock().expect("watchers lock");
        if let Some(senders) = watchers.get_mut(agent) {
            senders.retain(|tx| {
                tx.send(WatchEvent::Message {
                    agent: agent.to_owned(),
                    id,
                })
                .is_ok()
            });
            if senders.is_empty() {
                watchers.remove(agent);
            }
        }
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
    let mut stream =
        UnixStream::connect(socket_path).map_err(|source| connect_error(socket_path, source))?;
    serde_json::to_writer(&mut stream, request)?;
    stream.write_all(b"\n").map_err(|source| DaemonError::Io {
        path: socket_path.to_path_buf(),
        source,
    })?;
    read_response(&stream)
}

/// Open a `watch` subscription and invoke `on_event` for each wake
/// notification until the daemon closes the stream.
pub fn watch(
    socket_path: &Path,
    agent: &str,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<(), DaemonError> {
    watch_until(socket_path, agent, |event| {
        on_event(event);
        true
    })
}

/// Open a `watch` subscription and invoke `on_event` for each wake
/// notification until the daemon closes the stream or the callback returns
/// `false`.
pub fn watch_until(
    socket_path: &Path,
    agent: &str,
    mut on_event: impl FnMut(WatchEvent) -> bool,
) -> Result<(), DaemonError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|source| connect_error(socket_path, source))?;
    let request = DaemonRequest::Watch {
        agent: agent.to_owned(),
    };
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n").map_err(|source| DaemonError::Io {
        path: socket_path.to_path_buf(),
        source,
    })?;

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = line.map_err(|source| DaemonError::Io {
            path: socket_path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        if !on_event(serde_json::from_str(&line)?) {
            break;
        }
    }
    Ok(())
}

fn mailboxes_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("mailboxes")
}

fn connect_error(socket_path: &Path, source: std::io::Error) -> DaemonError {
    DaemonError::Connect {
        socket: socket_path.to_path_buf(),
        data_dir: socket_path
            .parent()
            .unwrap_or_else(|| Path::new(".aerial"))
            .to_path_buf(),
        source,
    }
}

fn restore_state(data_dir: &Path) -> Result<DaemonState, DaemonError> {
    let mut historical: HashMap<String, (AgentId, u64)> = HashMap::new();
    for message in Transcript::open(data_dir.join("history.jsonl"))?.messages()? {
        let sent_at = message.envelope.sent_at;
        restore_agent(
            &mut historical,
            message.from_name,
            message.envelope.from,
            sent_at,
        );
        restore_agent(
            &mut historical,
            message.to_name,
            message.envelope.to,
            sent_at,
        );
    }

    let mut restored = HashMap::new();
    for entry in fs::read_dir(mailboxes_dir(data_dir)).map_err(|source| DaemonError::Io {
        path: mailboxes_dir(data_dir),
        source,
    })? {
        let entry = entry.map_err(|source| DaemonError::Io {
            path: mailboxes_dir(data_dir),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let last_seen = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .and_then(|duration| duration.as_millis().try_into().ok())
            .unwrap_or(0);
        let (id, historical_last_seen) = historical
            .remove(name)
            .unwrap_or_else(|| (AgentId::new(), 0));
        restored.insert(name.to_owned(), (id, last_seen.max(historical_last_seen)));
    }

    let mut state = DaemonState::default();
    for (name, (id, last_seen)) in restored {
        state.known_agents.insert(name.clone(), id.clone());
        state.registry.restore(name, id, last_seen);
    }
    Ok(state)
}

fn restore_agent(
    restored: &mut HashMap<String, (AgentId, u64)>,
    name: String,
    id: AgentId,
    last_seen: u64,
) {
    match restored.get(&name) {
        Some((_, existing_last_seen)) if *existing_last_seen > last_seen => {}
        _ => {
            restored.insert(name, (id, last_seen));
        }
    }
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

fn write_event(stream: &mut UnixStream, event: &WatchEvent) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut *stream, event)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_sends_to_named_mailbox_and_acks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");

        let sent = daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "status?".to_owned(),
                in_reply_to: None,
                create: true,
            })
            .expect("send");

        let envelope = match sent {
            DaemonResponse::Sent { envelope } => envelope,
            other => panic!("unexpected response: {other:?}"),
        };

        assert_eq!(envelope.from_name.as_deref(), Some("engineer"));
        assert_eq!(envelope.to_name.as_deref(), Some("researcher"));

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
    fn daemon_rejects_unknown_recipient_without_create() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");

        let error = daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "ghost".to_owned(),
                body: "hello?".to_owned(),
                in_reply_to: None,
                create: false,
            })
            .expect_err("unknown recipient should fail");

        assert!(matches!(error, DaemonError::UnknownRecipient(name) if name == "ghost"));
    }

    #[test]
    fn connection_error_includes_startup_remedy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("missing.sock");
        let error = request(&socket, &DaemonRequest::Agents).expect_err("connection should fail");
        let message = error.to_string();

        assert!(message.contains("cannot connect to the Aerial daemon"));
        assert!(message.contains("aerial up --data-dir"));
        assert!(message.contains(&dir.path().display().to_string()));
    }

    #[test]
    fn registered_recipient_is_restored_after_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");
        daemon
            .handle(DaemonRequest::Register {
                name: "researcher".to_owned(),
            })
            .expect("register");
        drop(daemon);

        let restarted = Daemon::new(dir.path()).expect("restarted daemon");
        restarted
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "still there?".to_owned(),
                in_reply_to: None,
                create: false,
            })
            .expect("send to restored recipient");

        let agents = restarted
            .handle(DaemonRequest::Agents)
            .expect("list agents");
        assert!(matches!(
            agents,
            DaemonResponse::Agents { ref agents }
                if agents.len() == 1
                    && agents[0].name == "researcher"
                    && agents[0].pending == 1
        ));
    }

    #[test]
    fn history_sender_is_not_restored_as_a_registered_recipient() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");
        daemon
            .handle(DaemonRequest::Send {
                from: "sender-only".to_owned(),
                to: "researcher".to_owned(),
                body: "hello".to_owned(),
                in_reply_to: None,
                create: true,
            })
            .expect("initial send");
        drop(daemon);

        let restarted = Daemon::new(dir.path()).expect("restarted daemon");
        let error = restarted
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "sender-only".to_owned(),
                body: "should fail".to_owned(),
                in_reply_to: None,
                create: false,
            })
            .expect_err("sender-only name should not become a recipient");

        assert!(matches!(error, DaemonError::UnknownRecipient(name) if name == "sender-only"));
    }

    #[test]
    fn daemon_records_send_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");

        daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "please inspect the docs".to_owned(),
                in_reply_to: None,
                create: true,
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

    #[test]
    fn watch_is_notified_on_new_mail() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");

        // Subscribe a watcher for "researcher" by hand (mirrors what
        // serve_watch does over a socket).
        let (tx, rx) = mpsc::channel::<WatchEvent>();
        daemon
            .watchers
            .lock()
            .expect("watchers lock")
            .entry("researcher".to_owned())
            .or_default()
            .push(tx);

        let sent = daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "wake up".to_owned(),
                in_reply_to: None,
                create: true,
            })
            .expect("send");
        let id = match sent {
            DaemonResponse::Sent { envelope } => envelope.id,
            other => panic!("unexpected response: {other:?}"),
        };

        let event = rx.recv().expect("watch event");
        assert_eq!(
            event,
            WatchEvent::Message {
                agent: "researcher".to_owned(),
                id,
            }
        );
    }

    #[test]
    fn watch_for_other_agent_is_not_notified() {
        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");

        let (tx, rx) = mpsc::channel::<WatchEvent>();
        daemon
            .watchers
            .lock()
            .expect("watchers lock")
            .entry("someone_else".to_owned())
            .or_default()
            .push(tx);

        daemon
            .handle(DaemonRequest::Send {
                from: "engineer".to_owned(),
                to: "researcher".to_owned(),
                body: "not for you".to_owned(),
                in_reply_to: None,
                create: true,
            })
            .expect("send");

        assert!(rx.try_recv().is_err());
    }
}
