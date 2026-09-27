use std::io::{BufRead, BufReader, BufWriter, Write};
use std::sync::Arc;

use anastasia_client::DaemonClient;
use anastasia_protocol::{
    Command, DAEMON_ADDRESS_ENV, DAEMON_TOKEN_ENV, ResponsePayload,
};
use anyhow::{Context as _, anyhow, bail};
use parking_lot::Mutex;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub const SESSION_ID_ENV: &str = "ANASTASIA_SESSION_ID";

const SERVER_INSTRUCTIONS: &str = "Use these tools to query workspace context, standing rules, search context, and inspect tasks from Alabasta.";

pub fn serve_stdio() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve(BufReader::new(stdin.lock()), BufWriter::new(stdout.lock()))
}

struct BridgeClient {
    address: String,
    token: String,
    session_id: Uuid,
    daemon_client: Option<DaemonClient>,
}

impl BridgeClient {
    fn from_env() -> anyhow::Result<Self> {
        let address = std::env::var(DAEMON_ADDRESS_ENV)
            .context(format!("{DAEMON_ADDRESS_ENV} environment variable is required"))?;
        let token = std::env::var(DAEMON_TOKEN_ENV)
            .context(format!("{DAEMON_TOKEN_ENV} environment variable is required"))?;
        let session_id = std::env::var(SESSION_ID_ENV)
            .ok()
            .and_then(|id| Uuid::parse_str(&id).ok())
            .unwrap_or_else(Uuid::nil);

        Ok(Self {
            address,
            token,
            session_id,
            daemon_client: None,
        })
    }

    fn client(&mut self) -> anyhow::Result<&DaemonClient> {
        if self.daemon_client.is_none() {
            let client = DaemonClient::connect(&self.address, self.token.clone())
                .context("could not connect to Anastasia daemon")?;
            self.daemon_client = Some(client);
        }
        Ok(self.daemon_client.as_ref().unwrap())
    }

    fn call_tool(&mut self, tool: &str, arguments: JsonValue) -> anyhow::Result<JsonValue> {
        let session_id = self.session_id;
        let command = Command::AlabastaToolCall {
            tool: tool.to_string(),
            arguments,
        };
        let response = match self.client() {
            Ok(client) => client.request(session_id, Uuid::nil(), command),
            Err(_) => {
                // Try reconnecting once
                self.daemon_client = None;
                self.client()?.request(session_id, Uuid::nil(), command)
            }
        }?;

        match response {
            ResponsePayload::AlabastaToolResult { result } => Ok(result),
            other => bail!("unexpected daemon response: {other:?}"),
        }
    }
}

pub fn serve<R: BufRead, W: Write>(mut input: R, mut output: W) -> anyhow::Result<()> {
    let bridge = Arc::new(Mutex::new(BridgeClient::from_env().ok()));
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = input.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if line.len() > MAX_REQUEST_BYTES {
            write_message(
                &mut output,
                &json_rpc_error(JsonValue::Null, -32600, "MCP request is too large"),
            )?;
            continue;
        }
        let message: JsonValue = match serde_json::from_str(line.trim()) {
            Ok(message) => message,
            Err(error) => {
                write_message(
                    &mut output,
                    &json_rpc_error(JsonValue::Null, -32700, &format!("invalid JSON: {error}")),
                )?;
                continue;
            }
        };
        let Some(method) = message.get("method").and_then(JsonValue::as_str) else {
            if message.get("id").is_some() {
                write_message(
                    &mut output,
                    &json_rpc_error(
                        message.get("id").cloned().unwrap_or(JsonValue::Null),
                        -32600,
                        "JSON-RPC method is required",
                    ),
                )?;
            }
            continue;
        };
        if message.get("id").is_none() {
            if method == "exit" {
                break;
            }
            continue;
        }
        let id = message.get("id").cloned().unwrap_or(JsonValue::Null);
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        let response = match method {
            "initialize" => json_rpc_result(id, initialize_result()),
            "ping" => json_rpc_result(id, json!({})),
            "tools/list" => json_rpc_result(id, json!({"tools": tool_definitions()})),
            "tools/call" => match handle_tool_call(&bridge, &params) {
                Ok(result) => json_rpc_result(id, result),
                Err(error) => json_rpc_error(id, -32602, &error.to_string()),
            },
            "shutdown" => json_rpc_result(id, JsonValue::Null),
            _ => json_rpc_error(id, -32601, &format!("unsupported MCP method: {method}")),
        };
        write_message(&mut output, &response)?;
        if method == "shutdown" {
            break;
        }
    }
    Ok(())
}

fn initialize_result() -> JsonValue {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {
            "name": "anastasia_alabasta_bridge",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": SERVER_INSTRUCTIONS
    })
}

pub fn tool_definitions() -> Vec<JsonValue> {
    vec![
        json!({
            "name": "alabasta_get_context_package",
            "description": "Retrieve the compiled L1 context package for an Alabasta task, including relevant decisions, conventions, and dependencies.",
            "annotations": read_only_annotations(),
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "taskId": {
                        "type": "string",
                        "description": "The task ID (e.g. j57...) or identifier to retrieve context for."
                    }
                },
                "required": ["taskId"]
            }
        }),
        json!({
            "name": "alabasta_get_standing_context",
            "description": "Retrieve the L0 standing context rules for the workspace or a specific product.",
            "annotations": read_only_annotations(),
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "productId": {
                        "type": "string",
                        "description": "Optional product ID to scope standing rules."
                    }
                }
            }
        }),
        json!({
            "name": "alabasta_search_context",
            "description": "Search the workspace context using ranked keyword search.",
            "annotations": read_only_annotations(),
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query string."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Optional maximum number of search results to return."
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "alabasta_read_resource",
            "description": "Read an Alabasta context resource by its alabasta:// URI.",
            "annotations": read_only_annotations(),
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "uri": {
                        "type": "string",
                        "description": "The alabasta:// URI of the resource to read."
                    }
                },
                "required": ["uri"]
            }
        }),
        json!({
            "name": "alabasta_get_task",
            "description": "Get details for an Alabasta task by its human-readable identifier (e.g. ALB-482).",
            "annotations": read_only_annotations(),
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "identifier": {
                        "type": "string",
                        "description": "The human-readable task identifier, e.g. ALB-482."
                    }
                },
                "required": ["identifier"]
            }
        }),
    ]
}

fn read_only_annotations() -> JsonValue {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": true
    })
}

fn handle_tool_call(
    bridge: &Arc<Mutex<Option<BridgeClient>>>,
    params: &JsonValue,
) -> anyhow::Result<JsonValue> {
    let object = params
        .as_object()
        .ok_or_else(|| anyhow!("tools/call params must be an object"))?;
    let name = object
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("tools/call requires a tool name"))?;
    let arguments = object
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let mut guard = bridge.lock();
    if guard.is_none() {
        *guard = BridgeClient::from_env().ok();
    }
    let Some(client) = guard.as_mut() else {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": "Alabasta bridge is not connected to Anastasia daemon (daemon environment missing)"
            }],
            "isError": true
        }));
    };

    match client.call_tool(name, arguments) {
        Ok(result) => {
            let formatted = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": formatted
                }],
                "isError": false
            }))
        }
        Err(error) => Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Alabasta error: {error:#}")
            }],
            "isError": true
        })),
    }
}

fn json_rpc_result(id: JsonValue, result: JsonValue) -> JsonValue {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: JsonValue, code: i32, message: &str) -> JsonValue {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn write_message<W: Write>(output: &mut W, message: &JsonValue) -> anyhow::Result<()> {
    let line = serde_json::to_string(message)?;
    output.write_all(line.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_match_expected_names() {
        let defs = tool_definitions();
        let names = defs
            .iter()
            .filter_map(|d| d.get("name")?.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "alabasta_get_context_package",
                "alabasta_get_standing_context",
                "alabasta_search_context",
                "alabasta_read_resource",
                "alabasta_get_task",
            ]
        );
        for definition in defs {
            assert_eq!(definition["annotations"], read_only_annotations());
        }
    }
}
