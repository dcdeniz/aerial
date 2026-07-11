use std::path::{Path, PathBuf};

use aerial::daemon;
use aerial::{Daemon, DaemonRequest, DaemonResponse, Mailbox, SupervisorOptions, WatchEvent};
use anyhow::Context;
use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "aerial")]
#[command(about = "Durable, resumable messaging for local AI agents.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local Aerial daemon.
    #[command(visible_alias = "up")]
    Serve {
        /// Directory for the daemon socket and durable mailboxes.
        #[arg(long, default_value = ".aerial")]
        data_dir: PathBuf,
    },
    /// Register an agent name with a running daemon.
    #[command(visible_alias = "join")]
    Register {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Human-readable agent name.
        name: String,
    },
    /// Send a message through a running daemon.
    #[command(visible_alias = "send")]
    Tell {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Sender agent name.
        #[arg(long)]
        from: String,
        /// Recipient agent name.
        #[arg(long)]
        to: String,
        /// Message body to store in the envelope payload.
        #[arg(long)]
        body: String,
        /// Optional parent envelope id for lineage tracking.
        #[arg(long)]
        in_reply_to: Option<Uuid>,
    },
    /// List pending messages for an agent through a running daemon.
    #[command(visible_alias = "read")]
    Inbox {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Agent name.
        agent: String,
    },
    /// Acknowledge a message for an agent through a running daemon.
    #[command(visible_alias = "ack")]
    Done {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Agent name.
        #[arg(long)]
        agent: String,
        /// Envelope UUID to acknowledge.
        id: Uuid,
    },
    /// Show sent-message history across agents.
    #[command(visible_alias = "log")]
    History {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Number of most recent messages to show.
        #[arg(long)]
        limit: Option<usize>,
        /// Print the raw JSON response instead of the compact view.
        #[arg(long)]
        json: bool,
    },
    /// Show mailbox and recent history status in one command.
    Status {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Optional agent name whose pending mailbox should be shown.
        agent: Option<String>,
        /// Number of most recent history records to include.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Print the raw JSON response instead of the compact view.
        #[arg(long)]
        json: bool,
    },
    /// Acknowledge every pending message for an agent.
    Drain {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Agent name whose pending mailbox should be acknowledged.
        agent: String,
        /// Print the raw JSON response instead of the compact view.
        #[arg(long)]
        json: bool,
    },
    /// Register two agents, send a message, and show the recipient inbox.
    Exchange {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Sender agent name.
        #[arg(long)]
        from: String,
        /// Recipient agent name.
        #[arg(long)]
        to: String,
        /// Message body to store in the envelope payload.
        #[arg(long)]
        body: String,
        /// Optional parent envelope id for lineage tracking.
        #[arg(long)]
        in_reply_to: Option<Uuid>,
        /// Number of most recent history records to include.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Print the raw JSON response instead of the compact view.
        #[arg(long)]
        json: bool,
    },
    /// Serve the MCP adapter over stdio, translating MCP tool calls into daemon requests.
    #[command(name = "mcp", hide = true)]
    Mcp {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
    },
    /// Stream wake notifications for an agent, or run a hook on each.
    Watch {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Agent name to watch for incoming mail.
        agent: String,
        /// Shell command to run when a message arrives. The spawned process
        /// reads and acknowledges its own inbox; without it, events are
        /// printed as JSONL instead.
        #[arg(long)]
        exec: Option<String>,
    },
    /// Run an agent supervisor that wakes on mailbox messages.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Append a message to a local mailbox.
    #[command(name = "mailbox-send", hide = true)]
    Send {
        /// Path to the recipient mailbox JSONL file.
        #[arg(long)]
        mailbox: PathBuf,
        /// Message body to store in the envelope payload.
        #[arg(long)]
        body: String,
    },
    /// List unacknowledged messages in a local mailbox.
    #[command(name = "mailbox-pending", hide = true)]
    Pending {
        /// Path to the mailbox JSONL file.
        #[arg(long)]
        mailbox: PathBuf,
    },
    /// Acknowledge a message by envelope id.
    #[command(name = "mailbox-ack", hide = true)]
    Ack {
        /// Path to the mailbox JSONL file.
        #[arg(long)]
        mailbox: PathBuf,
        /// Envelope UUID to acknowledge.
        id: Uuid,
    },
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    /// Run an arbitrary command for each message and ack on successful exit.
    Exec {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Agent name to supervise.
        agent: String,
        /// Exit after handling one message.
        #[arg(long)]
        once: bool,
        /// Number of recent history records to expose to the supervisor.
        #[arg(long, default_value_t = 20)]
        history_limit: usize,
        /// Command and arguments to run for each message.
        #[arg(required = true, trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Run Codex for each message and ack on successful exit.
    Codex {
        /// Path to the daemon socket.
        #[arg(long, default_value = ".aerial/aerial.sock")]
        socket: PathBuf,
        /// Agent name to supervise.
        agent: String,
        /// Repository/workspace directory to pass to Codex.
        #[arg(long, default_value = ".")]
        cd: PathBuf,
        /// Codex approval policy.
        #[arg(long, default_value = "never")]
        ask_for_approval: String,
        /// Exit after handling one message.
        #[arg(long)]
        once: bool,
        /// Number of recent history records to include in the Codex prompt.
        #[arg(long, default_value_t = 20)]
        history_limit: usize,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { data_dir } => {
            let socket_path = data_dir.join("aerial.sock");
            Daemon::new(data_dir).context("create daemon")?.serve()?;
            println!("aerial daemon stopped: {}", socket_path.display());
        }
        Command::Register { socket, name } => {
            print_response(daemon::request(&socket, &DaemonRequest::Register { name })?)?;
        }
        Command::Tell {
            socket,
            from,
            to,
            body,
            in_reply_to,
        } => {
            print_response(daemon::request(
                &socket,
                &DaemonRequest::Send {
                    from,
                    to,
                    body,
                    in_reply_to,
                },
            )?)?;
        }
        Command::Inbox { socket, agent } => {
            print_response(daemon::request(&socket, &DaemonRequest::Pending { agent })?)?;
        }
        Command::Done { socket, agent, id } => {
            print_response(daemon::request(&socket, &DaemonRequest::Ack { agent, id })?)?;
        }
        Command::History {
            socket,
            limit,
            json,
        } => {
            let response = daemon::request(&socket, &DaemonRequest::History { limit })?;
            if json {
                print_response(response)?;
            } else {
                print_history(response)?;
            }
        }
        Command::Status {
            socket,
            agent,
            limit,
            json,
        } => {
            let report = status_report(&socket, agent.as_deref(), Some(limit))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_status_report(&report)?;
            }
        }
        Command::Drain {
            socket,
            agent,
            json,
        } => {
            let report = drain_agent(&socket, &agent)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Agent {agent}: acked {} pending message(s)",
                    report.acked.len()
                );
                for id in report.acked {
                    println!("{id}");
                }
            }
        }
        Command::Exchange {
            socket,
            from,
            to,
            body,
            in_reply_to,
            limit,
            json,
        } => {
            let report = exchange_report(&socket, &from, &to, &body, in_reply_to, Some(limit))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Agent {} -> Agent {} \"{}\"",
                    report.sent.from.short(),
                    report.sent.to.short(),
                    body.replace('\n', " ")
                );
                println!("Agent {to}: {} pending message(s)", report.pending.len());
                for envelope in &report.pending {
                    println!("{}", envelope.id);
                }
                for message in report.history {
                    println!("{}", message.render_summary());
                }
            }
        }
        Command::Mcp { socket } => {
            aerial::mcp::serve_stdio(socket).context("serve mcp adapter")?;
        }
        Command::Watch {
            socket,
            agent,
            exec,
        } => {
            daemon::watch(&socket, &agent, |event| match &exec {
                Some(command) => {
                    let WatchEvent::Message { agent, id } = &event;
                    eprintln!("aerial: message {id} for {agent}; running hook");
                    if let Err(error) = run_exec_hook(command, &socket, agent, *id) {
                        eprintln!("aerial: exec hook error: {error}");
                    }
                }
                None => match serde_json::to_string(&event) {
                    Ok(line) => println!("{line}"),
                    Err(error) => eprintln!("aerial: failed to encode event: {error}"),
                },
            })?;
        }
        Command::Agent { command } => match command {
            AgentCommand::Exec {
                socket,
                agent,
                once,
                history_limit,
                command,
            } => {
                let options = SupervisorOptions {
                    socket: socket.clone(),
                    agent: agent.clone(),
                    history_limit: Some(history_limit),
                };
                daemon::watch_until(&socket, &agent, |event| {
                    let WatchEvent::Message { id, .. } = event;
                    eprintln!("aerial: message {id} for {agent}; running agent command");
                    match aerial::supervisor::run_exec_agent(&options, id, &command) {
                        Ok(status) if status.success() => {
                            eprintln!("aerial: message {id} handled and acked");
                        }
                        Ok(status) => {
                            eprintln!(
                                "aerial: agent command exited with {status}; message {id} left pending"
                            );
                        }
                        Err(error) => {
                            eprintln!("aerial: agent command error: {error}");
                        }
                    }
                    !once
                })?;
            }
            AgentCommand::Codex {
                socket,
                agent,
                cd,
                ask_for_approval,
                once,
                history_limit,
            } => {
                daemon::watch_until(&socket, &agent, |event| {
                    let WatchEvent::Message { id, .. } = event;
                    eprintln!("aerial: message {id} for {agent}; running codex");
                    let command = aerial::supervisor::codex_command(
                        &socket,
                        &agent,
                        id,
                        &cd,
                        &ask_for_approval,
                        Some(history_limit),
                    );
                    match command.and_then(|command| {
                        let options = SupervisorOptions {
                            socket: socket.clone(),
                            agent: agent.clone(),
                            history_limit: Some(history_limit),
                        };
                        aerial::supervisor::run_exec_agent(&options, id, &command)
                    }) {
                        Ok(status) if status.success() => {
                            eprintln!("aerial: message {id} handled by codex and acked");
                        }
                        Ok(status) => {
                            eprintln!(
                                "aerial: codex exited with {status}; message {id} left pending"
                            );
                        }
                        Err(error) => {
                            eprintln!("aerial: codex supervisor error: {error}");
                        }
                    }
                    !once
                })?;
            }
        },
        Command::Send { mailbox, body } => {
            let mailbox = Mailbox::open(&mailbox).context("open mailbox")?;
            let envelope = aerial::Envelope::new(
                aerial::AgentId::new(),
                aerial::AgentId::new(),
                aerial::MessageKind::Message,
                serde_json::json!({ "body": body }),
            );
            mailbox.enqueue(&envelope).context("enqueue envelope")?;
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        Command::Pending { mailbox } => {
            let mailbox = Mailbox::open(&mailbox).context("open mailbox")?;
            let pending = mailbox.pending().context("read pending envelopes")?;
            println!("{}", serde_json::to_string_pretty(&pending)?);
        }
        Command::Ack { mailbox, id } => {
            let mailbox = Mailbox::open(&mailbox).context("open mailbox")?;
            mailbox.ack(id).context("ack envelope")?;
        }
    }

    Ok(())
}

fn print_response(response: DaemonResponse) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn print_history(response: DaemonResponse) -> anyhow::Result<()> {
    match response {
        DaemonResponse::History { messages } => {
            for message in messages {
                println!("{}", message.render_summary());
            }
            Ok(())
        }
        other => print_response(other),
    }
}

#[derive(serde::Serialize)]
struct StatusReport {
    agent: Option<String>,
    pending: Vec<aerial::Envelope>,
    history: Vec<aerial::TranscriptMessage>,
}

#[derive(serde::Serialize)]
struct DrainReport {
    agent: String,
    acked: Vec<Uuid>,
}

#[derive(serde::Serialize)]
struct ExchangeReport {
    from: String,
    to: String,
    sent: aerial::Envelope,
    pending: Vec<aerial::Envelope>,
    history: Vec<aerial::TranscriptMessage>,
}

fn status_report(
    socket: &Path,
    agent: Option<&str>,
    limit: Option<usize>,
) -> anyhow::Result<StatusReport> {
    let pending = match agent {
        Some(agent) => match daemon::request(
            socket,
            &DaemonRequest::Pending {
                agent: agent.to_owned(),
            },
        )? {
            DaemonResponse::Pending { envelopes } => envelopes,
            other => anyhow::bail!("unexpected pending response: {other:?}"),
        },
        None => Vec::new(),
    };
    let history = match daemon::request(socket, &DaemonRequest::History { limit })? {
        DaemonResponse::History { messages } => messages,
        other => anyhow::bail!("unexpected history response: {other:?}"),
    };
    Ok(StatusReport {
        agent: agent.map(str::to_owned),
        pending,
        history,
    })
}

fn print_status_report(report: &StatusReport) -> anyhow::Result<()> {
    if let Some(agent) = &report.agent {
        println!("Agent {agent}: {} pending message(s)", report.pending.len());
        for envelope in &report.pending {
            println!("{}", envelope.id);
        }
    }
    for message in &report.history {
        println!("{}", message.render_summary());
    }
    Ok(())
}

fn drain_agent(socket: &Path, agent: &str) -> anyhow::Result<DrainReport> {
    let pending = match daemon::request(
        socket,
        &DaemonRequest::Pending {
            agent: agent.to_owned(),
        },
    )? {
        DaemonResponse::Pending { envelopes } => envelopes,
        other => anyhow::bail!("unexpected pending response: {other:?}"),
    };
    let mut acked = Vec::new();
    for envelope in pending {
        match daemon::request(
            socket,
            &DaemonRequest::Ack {
                agent: agent.to_owned(),
                id: envelope.id,
            },
        )? {
            DaemonResponse::Acked { id } => acked.push(id),
            other => anyhow::bail!("unexpected ack response: {other:?}"),
        }
    }
    Ok(DrainReport {
        agent: agent.to_owned(),
        acked,
    })
}

fn exchange_report(
    socket: &Path,
    from: &str,
    to: &str,
    body: &str,
    in_reply_to: Option<Uuid>,
    limit: Option<usize>,
) -> anyhow::Result<ExchangeReport> {
    daemon::request(
        socket,
        &DaemonRequest::Register {
            name: from.to_owned(),
        },
    )?;
    daemon::request(
        socket,
        &DaemonRequest::Register {
            name: to.to_owned(),
        },
    )?;
    let sent = match daemon::request(
        socket,
        &DaemonRequest::Send {
            from: from.to_owned(),
            to: to.to_owned(),
            body: body.to_owned(),
            in_reply_to,
        },
    )? {
        DaemonResponse::Sent { envelope } => envelope,
        other => anyhow::bail!("unexpected send response: {other:?}"),
    };
    let status = status_report(socket, Some(to), limit)?;
    Ok(ExchangeReport {
        from: from.to_owned(),
        to: to.to_owned(),
        sent,
        pending: status.pending,
        history: status.history,
    })
}

/// Run a wake hook for a single arrived message. The command runs through the
/// platform shell and inherits stdio, so the hooked process can read and ack
/// its own inbox. The agent name, message id, and socket path are exposed as
/// environment variables for convenience.
fn run_exec_hook(command: &str, socket: &Path, agent: &str, id: Uuid) -> anyhow::Result<()> {
    let mut hook = if cfg!(windows) {
        let mut shell = std::process::Command::new("cmd");
        shell.arg("/C").arg(command);
        shell
    } else {
        let mut shell = std::process::Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    };
    let status = hook
        .env("AERIAL_AGENT", agent)
        .env("AERIAL_MESSAGE_ID", id.to_string())
        .env("AERIAL_SOCKET", socket)
        .status()
        .with_context(|| format!("run exec hook: {command}"))?;
    if !status.success() {
        eprintln!("aerial: exec hook for {agent} exited with status {status}");
    }
    Ok(())
}
