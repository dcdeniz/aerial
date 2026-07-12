pub mod daemon;
pub mod envelope;
pub mod mailbox;
pub mod mcp;
pub mod protocol;
pub mod registry;
pub mod supervisor;
pub mod transcript;

pub use daemon::{Daemon, DaemonError};
pub use envelope::{AgentId, Envelope, MessageKind};
pub use mailbox::{Mailbox, MailboxError};
pub use protocol::{AgentStatus, DaemonRequest, DaemonResponse, WatchEvent};
pub use registry::{RegisteredAgent, Registry};
pub use supervisor::{AgentMessage, SupervisorError, SupervisorOptions};
pub use transcript::{Transcript, TranscriptError, TranscriptMessage};
