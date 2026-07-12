use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::envelope::Envelope;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptMessage {
    pub envelope: Envelope,
    pub from_name: String,
    pub to_name: String,
}

impl TranscriptMessage {
    pub fn render_summary(&self) -> String {
        format!(
            "Agent {} -> Agent {} \"{}\"",
            self.from_name,
            self.to_name,
            summarize_body(&self.envelope)
        )
    }
}

#[derive(Debug, Error)]
pub enum TranscriptError {
    #[error("transcript io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid transcript record at {path}:{line}: {source}")]
    InvalidRecord {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
    #[error("failed to encode transcript record: {0}")]
    Encode(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
pub struct Transcript {
    path: PathBuf,
}

impl Transcript {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, TranscriptError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| TranscriptError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| TranscriptError::Io {
                path: path.clone(),
                source,
            })?;
        Ok(Self { path })
    }

    pub fn append(&self, message: &TranscriptMessage) -> Result<(), TranscriptError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| TranscriptError::Io {
                path: self.path.clone(),
                source,
            })?;
        serde_json::to_writer(&mut file, message)?;
        file.write_all(b"\n")
            .map_err(|source| TranscriptError::Io {
                path: self.path.clone(),
                source,
            })?;
        file.sync_data().map_err(|source| TranscriptError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }

    pub fn messages(&self) -> Result<Vec<TranscriptMessage>, TranscriptError> {
        let file = File::open(&self.path).map_err(|source| TranscriptError::Io {
            path: self.path.clone(),
            source,
        })?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(|source| TranscriptError::Io {
                path: self.path.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let message =
                serde_json::from_str(&line).map_err(|source| TranscriptError::InvalidRecord {
                    path: self.path.clone(),
                    line: idx + 1,
                    source,
                })?;
            messages.push(message);
        }

        Ok(messages)
    }

    pub fn tail(&self, limit: Option<usize>) -> Result<Vec<TranscriptMessage>, TranscriptError> {
        let messages = self.messages()?;
        let Some(limit) = limit else {
            return Ok(messages);
        };
        let start = messages.len().saturating_sub(limit);
        Ok(messages[start..].to_vec())
    }
}

fn summarize_body(envelope: &Envelope) -> String {
    let body = envelope
        .payload
        .get("body")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let mut summary: String = body.chars().take(50).collect();
    if body.chars().count() > 50 {
        summary.push_str(" ....");
    }
    summary
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::envelope::{AgentId, Envelope, MessageKind};

    use super::*;

    #[test]
    fn summary_truncates_body_to_history_view() {
        let envelope = Envelope::new(
            AgentId::new(),
            AgentId::new(),
            MessageKind::Message,
            json!({"body": "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"}),
        );
        let message = TranscriptMessage {
            envelope,
            from_name: "engineer".to_owned(),
            to_name: "researcher".to_owned(),
        };

        assert!(
            message
                .render_summary()
                .ends_with("\"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWX ....\"")
        );
        assert!(
            message
                .render_summary()
                .starts_with("Agent engineer -> Agent researcher")
        );
    }

    #[test]
    fn transcript_tails_messages() {
        let dir = tempfile::tempdir().expect("tempdir");
        let transcript = Transcript::open(dir.path().join("history.jsonl")).expect("open");

        for body in ["first", "second", "third"] {
            let message = TranscriptMessage {
                envelope: Envelope::new(
                    AgentId::new(),
                    AgentId::new(),
                    MessageKind::Message,
                    json!({ "body": body }),
                ),
                from_name: "a".to_owned(),
                to_name: "b".to_owned(),
            };
            transcript.append(&message).expect("append");
        }

        let tail = transcript.tail(Some(2)).expect("tail");

        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].envelope.payload["body"], "second");
        assert_eq!(tail[1].envelope.payload["body"], "third");
    }
}
