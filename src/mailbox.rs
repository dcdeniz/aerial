use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::envelope::Envelope;

#[derive(Debug, Error)]
pub enum MailboxError {
    #[error("mailbox io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid mailbox record at {path}:{line}: {source}")]
    InvalidRecord {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
    #[error("failed to encode mailbox record: {0}")]
    Encode(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
pub struct Mailbox {
    path: PathBuf,
}

impl Mailbox {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, MailboxError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| MailboxError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| MailboxError::Io {
                path: path.clone(),
                source,
            })?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn enqueue(&self, envelope: &Envelope) -> Result<(), MailboxError> {
        self.append(&MailboxRecord::Envelope {
            envelope: envelope.clone(),
        })
    }

    pub fn ack(&self, id: Uuid) -> Result<(), MailboxError> {
        self.append(&MailboxRecord::Ack { id })
    }

    pub fn pending(&self) -> Result<Vec<Envelope>, MailboxError> {
        let mut order = Vec::new();
        let mut envelopes = HashMap::new();
        let mut acked = HashSet::new();

        for record in self.records()? {
            match record {
                MailboxRecord::Envelope { envelope } => {
                    if !acked.contains(&envelope.id) && !envelopes.contains_key(&envelope.id) {
                        order.push(envelope.id);
                        envelopes.insert(envelope.id, envelope);
                    }
                }
                MailboxRecord::Ack { id } => {
                    acked.insert(id);
                    envelopes.remove(&id);
                }
            }
        }

        Ok(order
            .into_iter()
            .filter_map(|id| envelopes.remove(&id))
            .collect())
    }

    fn records(&self) -> Result<Vec<MailboxRecord>, MailboxError> {
        let file = File::open(&self.path).map_err(|source| MailboxError::Io {
            path: self.path.clone(),
            source,
        })?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(|source| MailboxError::Io {
                path: self.path.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record =
                serde_json::from_str(&line).map_err(|source| MailboxError::InvalidRecord {
                    path: self.path.clone(),
                    line: idx + 1,
                    source,
                })?;
            records.push(record);
        }

        Ok(records)
    }

    fn append(&self, record: &MailboxRecord) -> Result<(), MailboxError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| MailboxError::Io {
                path: self.path.clone(),
                source,
            })?;
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n").map_err(|source| MailboxError::Io {
            path: self.path.clone(),
            source,
        })?;
        file.sync_data().map_err(|source| MailboxError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MailboxRecord {
    Envelope { envelope: Envelope },
    Ack { id: Uuid },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::envelope::{AgentId, MessageKind};

    use super::*;

    #[test]
    fn pending_messages_survive_reopen_until_acked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("researcher.jsonl");
        let sender = AgentId::new();
        let recipient = AgentId::new();
        let first = Envelope::new(
            sender.clone(),
            recipient.clone(),
            MessageKind::Message,
            json!({"body": "first"}),
        );
        let second = Envelope::new(
            sender,
            recipient,
            MessageKind::Message,
            json!({"body": "second"}),
        );

        let mailbox = Mailbox::open(&path).expect("open mailbox");
        mailbox.enqueue(&first).expect("enqueue first");
        mailbox.enqueue(&second).expect("enqueue second");
        mailbox.ack(first.id).expect("ack first");

        let reopened = Mailbox::open(&path).expect("reopen mailbox");
        let pending = reopened.pending().expect("pending");

        assert_eq!(pending, vec![second]);
    }
}
