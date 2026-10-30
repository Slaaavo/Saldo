use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];

const INSTRUCTIONS: &str = "Saldo is the user's offline finance app. Call saldo_context first. \
Record documents with record_batch and dryRun true, and show the user the resulting table. Only \
after the user approved that table, call record_batch again with the same batch, dryRun false \
and the reviewToken the dry run returned.";

pub(super) trait ToolHost {
    fn list(&self) -> Vec<Value>;
    fn call(&self, name: &str, arguments: Value) -> ToolOutcome;
}

pub(super) enum ToolOutcome {
    Done(Value),
    Refused(Value),
    UnknownTool,
}

/// Serves newline-delimited JSON-RPC 2.0 until `input` ends. One line in, at most one line out.
/// Lines are read as bytes, so a line that is not UTF-8 gets a parse error like any other bad
/// line instead of ending the server.
pub(super) fn serve<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    host: &dyn ToolHost,
) -> std::io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        if line.trim_ascii().is_empty() {
            continue;
        }
        if let Some(response) = respond(&line, host) {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

fn respond(line: &[u8], host: &dyn ToolHost) -> Option<Value> {
    let Ok(message) = serde_json::from_slice::<Value>(line) else {
        return Some(error(Value::Null, -32700, "Parse error"));
    };
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        let is_client_response = message.get("result").is_some() || message.get("error").is_some();
        return (!is_client_response).then(|| error(Value::Null, -32600, "Invalid Request"));
    };
    // A notification has no id and never gets an answer.
    let id = message.get("id")?.clone();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => success(id, initialize(&params)),
        "ping" => success(id, json!({})),
        "tools/list" => success(id, json!({ "tools": host.list() })),
        "tools/call" => call_tool(id, &params, host),
        _ => error(id, -32601, &format!("Method not found: {method}")),
    })
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = PROTOCOL_VERSIONS
        .iter()
        .find(|version| Some(**version) == requested)
        .unwrap_or(&LATEST_PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "saldo", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn call_tool(id: Value, params: &Value, host: &dyn ToolHost) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error(id, -32602, "tools/call needs the tool's name");
    };
    let arguments = params
        .get("arguments")
        .filter(|arguments| !arguments.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let outcome =
        catch_unwind(AssertUnwindSafe(|| host.call(name, arguments))).unwrap_or_else(|_| {
            ToolOutcome::Refused(json!({
                "code": "INTERNAL",
                "message": "Saldo hit an internal error. Nothing was written.",
            }))
        });
    match outcome {
        ToolOutcome::Done(value) => success(id, tool_result(value, false)),
        ToolOutcome::Refused(value) => success(id, tool_result(value, true)),
        ToolOutcome::UnknownTool => error(id, -32602, &format!("Unknown tool: {name}")),
    }
}

fn tool_result(value: Value, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": value.to_string() }],
        "structuredContent": value,
        "isError": is_error,
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeHost;

    impl ToolHost for FakeHost {
        fn list(&self) -> Vec<Value> {
            vec![json!({ "name": "echo" })]
        }

        fn call(&self, name: &str, arguments: Value) -> ToolOutcome {
            match name {
                "echo" => ToolOutcome::Done(json!({ "echoed": arguments })),
                "refuse" => ToolOutcome::Refused(json!({ "code": "NO" })),
                "explode" => panic!("tool failed"),
                _ => ToolOutcome::UnknownTool,
            }
        }
    }

    /// Feeds `lines` to the server and returns every line it wrote, parsed.
    fn exchange(lines: &[&str]) -> Vec<Value> {
        let input = lines.join("\n");
        let mut output = Vec::new();
        serve(input.as_bytes(), &mut output, &FakeHost).expect("serve ends cleanly at EOF");
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn request(id: i64, method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
    }

    #[test]
    fn initialize_echoes_a_supported_protocol_version() {
        let replies = exchange(&[&request(
            1,
            "initialize",
            json!({ "protocolVersion": "2025-03-26" }),
        )]);

        assert_eq!(replies[0]["id"], 1);
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "saldo");
        assert!(replies[0]["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_answers_an_unknown_version_with_the_newest() {
        let replies = exchange(&[&request(
            1,
            "initialize",
            json!({ "protocolVersion": "1999-01-01" }),
        )]);

        assert_eq!(
            replies[0]["result"]["protocolVersion"],
            PROTOCOL_VERSIONS[0]
        );
    }

    #[test]
    fn notifications_get_no_reply() {
        let replies = exchange(&[
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#,
            &request(2, "ping", json!({})),
        ]);

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0]["id"], 2);
    }

    #[test]
    fn ping_answers_an_empty_result() {
        let replies = exchange(&[&request(7, "ping", Value::Null)]);

        assert_eq!(
            replies[0],
            json!({ "jsonrpc": "2.0", "id": 7, "result": {} })
        );
    }

    #[test]
    fn an_unknown_method_is_method_not_found() {
        let replies = exchange(&[&request(3, "resources/list", json!({}))]);

        assert_eq!(replies[0]["id"], 3);
        assert_eq!(replies[0]["error"]["code"], -32601);
    }

    #[test]
    fn a_malformed_line_is_a_parse_error_and_the_loop_goes_on() {
        let replies = exchange(&["{not json", &request(4, "ping", json!({}))]);

        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[0]["id"], Value::Null);
        assert_eq!(replies[1]["id"], 4);
    }

    #[test]
    fn a_line_that_is_not_utf8_is_a_parse_error_and_the_loop_goes_on() {
        let mut input =
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"x\":\"\xff\xfe\"}\n".to_vec();
        input.extend_from_slice(request(2, "ping", json!({})).as_bytes());
        let mut output = Vec::new();

        serve(input.as_slice(), &mut output, &FakeHost).expect("serve ends cleanly at EOF");

        let replies: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[1]["id"], 2);
    }

    #[test]
    fn responses_from_the_client_get_no_reply() {
        let replies = exchange(&[r#"{"jsonrpc":"2.0","id":9,"result":{}}"#]);

        assert!(replies.is_empty());
    }

    #[test]
    fn a_tool_result_carries_text_and_structured_content() {
        let replies = exchange(&[&request(
            5,
            "tools/call",
            json!({ "name": "echo", "arguments": { "a": 1 } }),
        )]);

        let result = &replies[0]["result"];
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"], json!({ "echoed": { "a": 1 } }));
        let text: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, result["structuredContent"]);
    }

    #[test]
    fn missing_arguments_reach_the_tool_as_an_empty_object() {
        let replies = exchange(&[&request(5, "tools/call", json!({ "name": "echo" }))]);

        assert_eq!(
            replies[0]["result"]["structuredContent"],
            json!({ "echoed": {} })
        );
    }

    #[test]
    fn a_tool_refusal_is_an_error_result_not_a_protocol_error() {
        let replies = exchange(&[&request(6, "tools/call", json!({ "name": "refuse" }))]);

        assert!(replies[0].get("error").is_none());
        assert_eq!(replies[0]["result"]["isError"], true);
        assert_eq!(replies[0]["result"]["structuredContent"]["code"], "NO");
    }

    #[test]
    fn a_panicking_tool_is_an_error_result_and_the_loop_goes_on() {
        let replies = exchange(&[
            &request(8, "tools/call", json!({ "name": "explode" })),
            &request(9, "ping", json!({})),
        ]);

        assert_eq!(replies[0]["result"]["isError"], true);
        assert_eq!(
            replies[0]["result"]["structuredContent"]["code"],
            "INTERNAL"
        );
        assert_eq!(replies[1]["id"], 9);
    }

    #[test]
    fn an_unknown_tool_is_invalid_params() {
        let replies = exchange(&[&request(10, "tools/call", json!({ "name": "nope" }))]);

        assert_eq!(replies[0]["error"]["code"], -32602);
    }

    #[test]
    fn eof_ends_the_loop_cleanly() {
        assert!(exchange(&[]).is_empty());
    }
}
