# MCP Adapter

Aerial exposes the running daemon to [MCP](https://modelcontextprotocol.io)
clients through a hidden subcommand:

```sh
aerial mcp --socket .aerial/aerial.sock
```

It speaks JSON-RPC 2.0 over stdio — newline-delimited JSON, the framing MCP's
stdio transport uses, which is also the line-delimited JSON the daemon already
speaks.

## Principle

The MCP adapter creates **no separate mailbox state**. Every tool call is
translated into a daemon request and dispatched over the socket, so the daemon
stays the single source of truth:

```text
MCP client -> aerial mcp (stdio) -> daemon request -> durable mailbox
```

If the daemon is not running, tool calls fail; the adapter never invents its own
storage.

## Tools

Primitive tools map 1:1 onto the daemon protocol:

| Tool | Arguments | Daemon action |
|------|-----------|---------------|
| `register` | `name` | Register an agent name. |
| `tell` | `from`, `to`, `body`, `in_reply_to?` | Send a message; `in_reply_to` is a parent envelope UUID for lineage. |
| `inbox` | `agent` | List pending (unacknowledged) messages. |
| `done` | `agent`, `id` | Acknowledge an envelope by UUID. |
| `history` | `limit?` | Show recent sent-message history. |

Macro tools bundle common Aerial flows while still dispatching only to the
running daemon:

| Tool | Arguments | Flow |
|------|-----------|------|
| `status` | `agent?`, `limit?` | Return an optional agent inbox plus recent history. |
| `drain` | `agent` | Acknowledge every pending message for an agent. |
| `exchange` | `from`, `to`, `body`, `in_reply_to?`, `limit?` | Register both names, send a message, then return the recipient inbox and recent history. |

Each call returns the daemon's JSON response as MCP text content. Tool-level
failures (bad arguments, a daemon error, or an unknown tool) come back as a
result with `isError: true`; malformed frames or unknown methods use JSON-RPC
error objects.

## Client configuration

Point an MCP client at the binary and pass the daemon socket. For a client that
launches stdio servers from a config file:

```json
{
  "mcpServers": {
    "aerial": {
      "command": "aerial",
      "args": ["mcp", "--socket", "/absolute/path/to/.aerial/aerial.sock"]
    }
  }
}
```

The daemon must already be running (`aerial up`) so the adapter has a socket to
dispatch to.

## Example session

Newline-delimited JSON-RPC in, newline-delimited JSON-RPC out (one response per
request; notifications get none):

```text
-> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}
<- {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"aerial","version":"0.4.1"}}}
-> {"jsonrpc":"2.0","method":"notifications/initialized"}
-> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"register","arguments":{"name":"alice"}}}
<- {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{ \"status\": \"registered\", ... }"}],"isError":false}}
-> {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tell","arguments":{"from":"alice","to":"bob","body":"hi bob"}}}
<- {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{ \"status\": \"sent\", ... }"}],"isError":false}}
-> {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"exchange","arguments":{"from":"alice","to":"bob","body":"macro hello","limit":5}}}
<- {"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"{ \"from\": \"alice\", ... }"}],"isError":false}}
```
