//! Minimal MCP (Model Context Protocol) adapter over stdio.
//!
//! Speaks JSON-RPC 2.0 over newline-delimited stdin/stdout — the framing MCP's
//! stdio transport uses, which is also the line-delimited JSON the daemon
//! already speaks. Every tool call is translated into a [`DaemonRequest`] and
//! dispatched through [`daemon::request`] to the running daemon, which stays the
//! single source of truth: this module keeps **no** mailbox state of its own.
//!
//! The primitive tools map 1:1 onto the daemon protocol:
//! `register` → Register, `tell` → Send, `inbox` → Pending, `done` → Ack,
//! `history` → History. Macro tools bundle those same daemon calls into common
//! flows without creating separate state.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::daemon;
use crate::protocol::{DaemonRequest, DaemonResponse};

/// MCP protocol revision this adapter implements by default. When a client
/// announces a different `protocolVersion` on `initialize`, we echo theirs back
/// for maximum compatibility with a deliberately minimal server.
const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the stdio MCP server against the daemon at `socket` until stdin closes.
///
/// Diagnostics must never go to stdout — that channel is reserved for protocol
/// frames — so errors surface as JSON-RPC error responses instead.
pub fn serve_stdio(socket: PathBuf) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(stdin.lock(), stdout.lock(), &socket)
}

/// Drive the MCP server loop over an arbitrary reader/writer against the daemon
/// at `socket`. Reads one JSON-RPC message per line, dispatches it, and writes
/// at most one response line back; notifications (messages without an `id`)
/// produce no response. Factored out of [`serve_stdio`] so the framing can be
/// exercised in tests with in-memory buffers.
pub fn serve(reader: impl BufRead, mut writer: impl Write, socket: &Path) -> io::Result<()> {
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => dispatch(socket, &message),
            Err(error) => Some(error_response(
                Value::Null,
                -32700,
                &format!("parse error: {error}"),
            )),
        };

        if let Some(response) = response {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

/// Route one JSON-RPC message, returning a response, or `None` for a
/// notification (which by spec expects no reply).
fn dispatch(socket: &Path, message: &Value) -> Option<Value> {
    let method = message.get("method").and_then(Value::as_str);

    // No `id` ⇒ this is a notification (e.g. `notifications/initialized`);
    // acknowledge silently by producing no response.
    let id = message.get("id").cloned()?;

    let response = match method {
        Some("initialize") => result_response(id, initialize_result(message.get("params"))),
        Some("tools/list") => result_response(id, json!({ "tools": tool_definitions() })),
        Some("tools/call") => handle_tool_call(socket, id, message.get("params")),
        Some("ping") => result_response(id, json!({})),
        Some(other) => error_response(id, -32601, &format!("method not found: {other}")),
        None => error_response(id, -32600, "invalid request: missing method"),
    };
    Some(response)
}

/// Build the `initialize` result, echoing the client's requested protocol
/// version when present.
fn initialize_result(params: Option<&Value>) -> Value {
    let version = params
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);

    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "aerial", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// Tools exposed to MCP clients. Primitive tools mirror daemon requests; macro
/// tools bundle common Aerial flows using the same daemon state.
fn tool_definitions() -> Value {
    json!([
        {
            "name": "register",
            "description": "Register an agent name with the running Aerial daemon.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Human-readable agent name." }
                },
                "required": ["name"]
            }
        },
        {
            "name": "tell",
            "description": "Send a message from one agent to another through the daemon.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "Sender agent name." },
                    "to": { "type": "string", "description": "Recipient agent name." },
                    "body": { "type": "string", "description": "Message body." },
                    "in_reply_to": {
                        "type": "string",
                        "description": "Optional parent envelope UUID for lineage tracking."
                    }
                },
                "required": ["from", "to", "body"]
            }
        },
        {
            "name": "inbox",
            "description": "List pending (unacknowledged) messages for an agent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent": { "type": "string", "description": "Agent name whose mailbox to read." }
                },
                "required": ["agent"]
            }
        },
        {
            "name": "done",
            "description": "Acknowledge (remove) a pending message from an agent's mailbox by envelope id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent": { "type": "string", "description": "Agent name whose message to acknowledge." },
                    "id": { "type": "string", "description": "Envelope UUID to acknowledge." }
                },
                "required": ["agent", "id"]
            }
        },
        {
            "name": "history",
            "description": "Show recent sent-message history across all agents.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of recent messages to return."
                    }
                }
            }
        },
        {
            "name": "status",
            "description": "Show an optional agent inbox plus recent sent-message history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "string",
                        "description": "Optional agent name whose pending mailbox should be included."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of recent history messages to return."
                    }
                }
            }
        },
        {
            "name": "drain",
            "description": "Acknowledge every pending message for an agent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "string",
                        "description": "Agent name whose pending mailbox should be acknowledged."
                    }
                },
                "required": ["agent"]
            }
        },
        {
            "name": "exchange",
            "description": "Register two agents, send a message, then return the recipient inbox and recent history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "Sender agent name." },
                    "to": { "type": "string", "description": "Recipient agent name." },
                    "body": { "type": "string", "description": "Message body." },
                    "in_reply_to": {
                        "type": "string",
                        "description": "Optional parent envelope UUID for lineage tracking."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of recent history messages to return."
                    }
                },
                "required": ["from", "to", "body"]
            }
        }
    ])
}

/// Handle a `tools/call` request: decode the tool + arguments into a
/// [`DaemonRequest`], dispatch it to the daemon, and wrap the outcome as MCP
/// tool content. Both bad arguments and daemon failures are reported as tool
/// results with `isError: true` (per MCP convention) rather than JSON-RPC
/// errors, so the calling model sees a readable explanation.
fn handle_tool_call(socket: &Path, id: Value, params: Option<&Value>) -> Value {
    let Some(params) = params else {
        return error_response(id, -32602, "invalid params: missing params");
    };

    let name = params.get("name").and_then(Value::as_str);
    let empty = json!({});
    let arguments = params.get("arguments").unwrap_or(&empty);

    match name {
        Some("status") => return handle_status(socket, id, arguments),
        Some("drain") => return handle_drain(socket, id, arguments),
        Some("exchange") => return handle_exchange(socket, id, arguments),
        _ => {}
    }

    let request = match build_request(name, arguments) {
        Ok(request) => request,
        Err(message) => return result_response(id, tool_error(&message)),
    };

    match daemon::request(socket, &request) {
        Ok(response) => result_response(id, tool_result(&response)),
        Err(error) => result_response(id, tool_error(&format!("daemon error: {error}"))),
    }
}

/// Translate a tool name + arguments object into a [`DaemonRequest`].
fn build_request(name: Option<&str>, args: &Value) -> Result<DaemonRequest, String> {
    match name {
        Some("register") => Ok(DaemonRequest::Register {
            name: required_str(args, "name")?,
        }),
        Some("tell") => Ok(DaemonRequest::Send {
            from: required_str(args, "from")?,
            to: required_str(args, "to")?,
            body: required_str(args, "body")?,
            in_reply_to: optional_uuid(args, "in_reply_to")?,
        }),
        Some("inbox") => Ok(DaemonRequest::Pending {
            agent: required_str(args, "agent")?,
        }),
        Some("done") => Ok(DaemonRequest::Ack {
            agent: required_str(args, "agent")?,
            id: required_uuid(args, "id")?,
        }),
        Some("history") => Ok(DaemonRequest::History {
            limit: optional_usize(args, "limit")?,
        }),
        Some(other) => Err(format!("unknown tool: {other}")),
        None => Err("missing tool name".to_owned()),
    }
}

fn handle_status(socket: &Path, id: Value, args: &Value) -> Value {
    let agent = optional_str(args, "agent");
    let limit = match optional_usize(args, "limit") {
        Ok(limit) => limit,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    match status(socket, agent.as_deref(), limit) {
        Ok(result) => result_response(id, json_tool_result(&result)),
        Err(message) => result_response(id, tool_error(&message)),
    }
}

fn handle_drain(socket: &Path, id: Value, args: &Value) -> Value {
    let agent = match required_str(args, "agent") {
        Ok(agent) => agent,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    match drain(socket, &agent) {
        Ok(result) => result_response(id, json_tool_result(&result)),
        Err(message) => result_response(id, tool_error(&message)),
    }
}

fn handle_exchange(socket: &Path, id: Value, args: &Value) -> Value {
    let from = match required_str(args, "from") {
        Ok(from) => from,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    let to = match required_str(args, "to") {
        Ok(to) => to,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    let body = match required_str(args, "body") {
        Ok(body) => body,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    let in_reply_to = match optional_uuid(args, "in_reply_to") {
        Ok(id) => id,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    let limit = match optional_usize(args, "limit") {
        Ok(limit) => limit,
        Err(message) => return result_response(id, tool_error(&message)),
    };
    match exchange(socket, &from, &to, &body, in_reply_to, limit) {
        Ok(result) => result_response(id, json_tool_result(&result)),
        Err(message) => result_response(id, tool_error(&message)),
    }
}

fn status(socket: &Path, agent: Option<&str>, limit: Option<usize>) -> Result<Value, String> {
    let pending = match agent {
        Some(agent) => match daemon::request(
            socket,
            &DaemonRequest::Pending {
                agent: agent.to_owned(),
            },
        ) {
            Ok(DaemonResponse::Pending { envelopes }) => json!(envelopes),
            Ok(other) => return Err(format!("unexpected pending response: {other:?}")),
            Err(error) => return Err(format!("daemon error: {error}")),
        },
        None => json!([]),
    };
    let history = match daemon::request(socket, &DaemonRequest::History { limit }) {
        Ok(DaemonResponse::History { messages }) => json!(messages),
        Ok(other) => return Err(format!("unexpected history response: {other:?}")),
        Err(error) => return Err(format!("daemon error: {error}")),
    };
    Ok(json!({
        "agent": agent,
        "pending": pending,
        "history": history
    }))
}

fn drain(socket: &Path, agent: &str) -> Result<Value, String> {
    let pending = match daemon::request(
        socket,
        &DaemonRequest::Pending {
            agent: agent.to_owned(),
        },
    ) {
        Ok(DaemonResponse::Pending { envelopes }) => envelopes,
        Ok(other) => return Err(format!("unexpected pending response: {other:?}")),
        Err(error) => return Err(format!("daemon error: {error}")),
    };
    let mut acked = Vec::new();
    for envelope in pending {
        match daemon::request(
            socket,
            &DaemonRequest::Ack {
                agent: agent.to_owned(),
                id: envelope.id,
            },
        ) {
            Ok(DaemonResponse::Acked { id }) => acked.push(id),
            Ok(other) => return Err(format!("unexpected ack response: {other:?}")),
            Err(error) => return Err(format!("daemon error: {error}")),
        }
    }
    Ok(json!({ "agent": agent, "acked": acked }))
}

fn exchange(
    socket: &Path,
    from: &str,
    to: &str,
    body: &str,
    in_reply_to: Option<Uuid>,
    limit: Option<usize>,
) -> Result<Value, String> {
    for name in [from, to] {
        if let Err(error) = daemon::request(
            socket,
            &DaemonRequest::Register {
                name: name.to_owned(),
            },
        ) {
            return Err(format!("daemon error: {error}"));
        }
    }
    let sent = match daemon::request(
        socket,
        &DaemonRequest::Send {
            from: from.to_owned(),
            to: to.to_owned(),
            body: body.to_owned(),
            in_reply_to,
        },
    ) {
        Ok(DaemonResponse::Sent { envelope }) => envelope,
        Ok(other) => return Err(format!("unexpected send response: {other:?}")),
        Err(error) => return Err(format!("daemon error: {error}")),
    };
    let status = status(socket, Some(to), limit)?;
    Ok(json!({
        "from": from,
        "to": to,
        "sent": sent,
        "status": status
    }))
}

/// Render a successful daemon response as MCP tool content. The pretty-printed
/// JSON response body is returned as a text block so the calling model sees the
/// full structured result (envelope ids, pending lists, etc.).
fn tool_result(response: &DaemonResponse) -> Value {
    let text = serde_json::to_string_pretty(response)
        .unwrap_or_else(|error| format!("<failed to serialize response: {error}>"));
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": matches!(response, DaemonResponse::Error { .. })
    })
}

fn json_tool_result(value: &Value) -> Value {
    let text = serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| format!("<failed to serialize response: {error}>"));
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": false
    })
}

/// Render a tool-level failure as MCP error content.
fn tool_error(message: &str) -> Value {
    json!({
        "content": [ { "type": "text", "text": message } ],
        "isError": true
    })
}

fn required_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing required string argument: {key}"))
}

fn optional_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn optional_uuid(args: &Value, key: &str) -> Result<Option<Uuid>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("argument {key} must be a string uuid"))?;
            Uuid::parse_str(text)
                .map(Some)
                .map_err(|error| format!("argument {key} is not a valid uuid: {error}"))
        }
    }
}

fn required_uuid(args: &Value, key: &str) -> Result<Uuid, String> {
    optional_uuid(args, key)?.ok_or_else(|| format!("missing required uuid argument: {key}"))
}

fn optional_usize(args: &Value, key: &str) -> Result<Option<usize>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(|count| Some(count as usize))
            .ok_or_else(|| format!("argument {key} must be a non-negative integer")),
    }
}

fn result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn write_message(out: &mut impl Write, message: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *out, message)?;
    out.write_all(b"\n")?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_echoes_client_protocol_version() {
        let params = json!({ "protocolVersion": "2025-06-18" });
        let result = initialize_result(Some(&params));
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert_eq!(result["serverInfo"]["name"], "aerial");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_falls_back_to_default_version() {
        let result = initialize_result(None);
        assert_eq!(result["protocolVersion"], DEFAULT_PROTOCOL_VERSION);
    }

    #[test]
    fn tools_list_exposes_daemon_and_macro_tools() {
        let tools = tool_definitions();
        let names: Vec<&str> = tools
            .as_array()
            .expect("tools array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect();
        assert_eq!(
            names,
            [
                "register", "tell", "inbox", "done", "history", "status", "drain", "exchange"
            ]
        );
    }

    #[test]
    fn notifications_get_no_response() {
        let socket = Path::new("unused.sock");
        let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(dispatch(socket, &notification).is_none());
    }

    #[test]
    fn unknown_method_is_a_json_rpc_error() {
        let socket = Path::new("unused.sock");
        let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "does/not/exist" });
        let response = dispatch(socket, &request).expect("response");
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn build_request_maps_register() {
        let request =
            build_request(Some("register"), &json!({ "name": "jeff" })).expect("register");
        assert_eq!(
            request,
            DaemonRequest::Register {
                name: "jeff".to_owned()
            }
        );
    }

    #[test]
    fn build_request_maps_tell_with_lineage() {
        let parent = Uuid::new_v4();
        let request = build_request(
            Some("tell"),
            &json!({ "from": "jeff", "to": "claude", "body": "hi", "in_reply_to": parent.to_string() }),
        )
        .expect("tell");
        assert_eq!(
            request,
            DaemonRequest::Send {
                from: "jeff".to_owned(),
                to: "claude".to_owned(),
                body: "hi".to_owned(),
                in_reply_to: Some(parent),
            }
        );
    }

    #[test]
    fn build_request_rejects_missing_required_argument() {
        let error = build_request(Some("tell"), &json!({ "from": "jeff" })).unwrap_err();
        assert!(error.contains("to"));
    }

    #[test]
    fn build_request_rejects_bad_uuid() {
        let error = build_request(
            Some("done"),
            &json!({ "agent": "jeff", "id": "not-a-uuid" }),
        )
        .unwrap_err();
        assert!(error.contains("uuid"));
    }

    #[test]
    fn build_request_maps_history_limit() {
        let request = build_request(Some("history"), &json!({ "limit": 5 })).expect("history");
        assert_eq!(request, DaemonRequest::History { limit: Some(5) });
    }

    // ---- Loop framing over in-memory buffers (no daemon needed) -------------

    #[test]
    fn stdio_loop_frames_one_response_per_request_and_skips_notifications() {
        use std::io::Cursor;

        // `initialize` and `tools/list` each expect one response line; the
        // `notifications/initialized` message and the blank line expect none.
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\"}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
        );
        let mut output = Vec::new();
        serve(Cursor::new(input), &mut output, Path::new("unused.sock")).expect("serve");

        let text = String::from_utf8(output).expect("utf8 output");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "exactly two responses expected");

        let initialize: Value = serde_json::from_str(lines[0]).expect("initialize json");
        assert_eq!(initialize["id"], 1);
        assert_eq!(initialize["result"]["protocolVersion"], "2025-06-18");

        let tools: Value = serde_json::from_str(lines[1]).expect("tools/list json");
        assert_eq!(tools["id"], 2);
        assert_eq!(
            tools["result"]["tools"]
                .as_array()
                .expect("tools array")
                .len(),
            8
        );
    }

    #[test]
    fn malformed_line_yields_parse_error_frame() {
        use std::io::Cursor;

        let mut output = Vec::new();
        serve(
            Cursor::new("{ not json\n"),
            &mut output,
            Path::new("unused.sock"),
        )
        .expect("serve");
        let response: Value = serde_json::from_slice(&output).expect("json");
        assert_eq!(response["error"]["code"], -32700);
    }

    // ---- End-to-end against a live in-process daemon -----------------------

    fn start_daemon() -> (tempfile::TempDir, std::path::PathBuf) {
        use crate::daemon::Daemon;
        use std::time::Duration;

        let dir = tempfile::tempdir().expect("tempdir");
        let daemon = Daemon::new(dir.path()).expect("daemon");
        let socket = daemon.socket_path();
        std::thread::spawn(move || {
            let _ = daemon.serve();
        });

        // Wait for the listener to start accepting before returning.
        for _ in 0..200 {
            if daemon::request(&socket, &DaemonRequest::History { limit: Some(1) }).is_ok() {
                return (dir, socket);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("daemon did not become ready");
    }

    fn call_tool(socket: &Path, id: i64, name: &str, arguments: Value) -> Value {
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        dispatch(socket, &message).expect("tool response")
    }

    /// Parse the daemon response JSON carried in a tool result's text content.
    fn tool_body(response: &Value) -> Value {
        assert_eq!(
            response["result"]["isError"], false,
            "tool reported error: {response}"
        );
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        serde_json::from_str(text).expect("daemon response json")
    }

    #[test]
    fn all_tools_round_trip_through_the_live_daemon() {
        let (_dir, socket) = start_daemon();

        let registered = call_tool(&socket, 1, "register", json!({ "name": "alice" }));
        assert_eq!(tool_body(&registered)["status"], "registered");

        let sent = call_tool(
            &socket,
            2,
            "tell",
            json!({ "from": "alice", "to": "bob", "body": "hi bob" }),
        );
        assert_eq!(tool_body(&sent)["status"], "sent");

        let inbox = call_tool(&socket, 3, "inbox", json!({ "agent": "bob" }));
        let pending = tool_body(&inbox);
        let envelopes = pending["envelopes"].as_array().expect("envelopes");
        assert_eq!(envelopes.len(), 1);
        assert_eq!(envelopes[0]["payload"]["body"], "hi bob");
        let envelope_id = envelopes[0]["id"].as_str().expect("envelope id").to_owned();

        let acked = call_tool(
            &socket,
            4,
            "done",
            json!({ "agent": "bob", "id": envelope_id }),
        );
        assert_eq!(tool_body(&acked)["status"], "acked");

        let inbox_after = call_tool(&socket, 5, "inbox", json!({ "agent": "bob" }));
        assert!(
            tool_body(&inbox_after)["envelopes"]
                .as_array()
                .expect("envelopes")
                .is_empty(),
            "mailbox should be drained after done"
        );

        let history = call_tool(&socket, 6, "history", json!({ "limit": 10 }));
        assert_eq!(tool_body(&history)["status"], "history");

        let exchange = call_tool(
            &socket,
            7,
            "exchange",
            json!({ "from": "alice", "to": "bob", "body": "macro hi", "limit": 10 }),
        );
        let exchange_body = tool_body(&exchange);
        assert_eq!(exchange_body["from"], "alice");
        assert_eq!(exchange_body["to"], "bob");
        assert_eq!(exchange_body["sent"]["payload"]["body"], "macro hi");

        let status = call_tool(&socket, 8, "status", json!({ "agent": "bob", "limit": 10 }));
        let status_body = tool_body(&status);
        assert_eq!(status_body["pending"][0]["payload"]["body"], "macro hi");

        let drain = call_tool(&socket, 9, "drain", json!({ "agent": "bob" }));
        let drain_body = tool_body(&drain);
        assert_eq!(drain_body["agent"], "bob");
        assert_eq!(drain_body["acked"].as_array().expect("acked").len(), 1);

        let status_after = call_tool(&socket, 10, "status", json!({ "agent": "bob" }));
        assert!(
            tool_body(&status_after)["pending"]
                .as_array()
                .expect("pending")
                .is_empty(),
            "mailbox should be drained after macro drain"
        );
    }

    #[test]
    fn tool_call_reports_unknown_tool_as_error() {
        let (_dir, socket) = start_daemon();
        let response = call_tool(&socket, 1, "nope", json!({}));
        assert_eq!(response["result"]["isError"], true);
    }
}
