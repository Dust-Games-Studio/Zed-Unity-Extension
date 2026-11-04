use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;

struct LspClient {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    next_id: i64,
    notifications: Vec<Value>,
}

impl LspClient {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_unity-asmdef-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn unity-asmdef-lsp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (sender, messages) = mpsc::channel();

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut length = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let line = line.trim_end();
                    if line.is_empty() {
                        break;
                    }
                    if let Some(value) = line.strip_prefix("Content-Length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                if length == 0 {
                    return;
                }
                let mut body = vec![0; length];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if let Ok(message) = serde_json::from_slice::<Value>(&body) {
                    let _ = sender.send(message);
                }
            }
        });

        Self {
            child,
            stdin,
            messages,
            next_id: 1,
            notifications: Vec::new(),
        }
    }

    fn send(&mut self, message: Value) {
        let body = serde_json::to_string(&message).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));

        loop {
            let message = self
                .messages
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| panic!("timed out waiting for {method}"));
            if message.get("id") == Some(&json!(id)) {
                if let Some(error) = message.get("error") {
                    panic!("{method} failed: {error}");
                }
                return message.get("result").cloned().unwrap_or(Value::Null);
            }
            self.notifications.push(message);
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }));
    }

    fn wait_for_diagnostics(&mut self, uri: &str) -> Vec<Value> {
        if let Some(diagnostics) = take_diagnostics(&mut self.notifications, uri) {
            return diagnostics;
        }

        loop {
            let message = self
                .messages
                .recv_timeout(Duration::from_secs(10))
                .expect("timed out waiting for diagnostics");
            if message.get("method") == Some(&json!("textDocument/publishDiagnostics"))
                && message["params"]["uri"] == uri
            {
                return message["params"]["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
            }
            self.notifications.push(message);
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn take_diagnostics(notifications: &mut Vec<Value>, uri: &str) -> Option<Vec<Value>> {
    let index = notifications.iter().position(|message| {
        message.get("method") == Some(&json!("textDocument/publishDiagnostics"))
            && message["params"]["uri"] == uri
    })?;
    let message = notifications.remove(index);
    Some(
        message["params"]["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    )
}

fn completion_items(value: &Value) -> Vec<&Value> {
    if let Some(items) = value.as_array() {
        items.iter().collect()
    } else {
        value["items"]
            .as_array()
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    }
}

fn definition_locations(value: &Value) -> Vec<&Value> {
    if let Some(locations) = value.as_array() {
        locations.iter().collect()
    } else if value.is_object() {
        vec![value]
    } else {
        Vec::new()
    }
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(
        &root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 6000.6.6f1",
    );
    write(
        &root.join("Packages/manifest.json"),
        r#"{ "dependencies": {} }"#,
    );
    write(
        &root.join("Packages/packages-lock.json"),
        r#"{ "dependencies": {} }"#,
    );
    write(
        &root.join("Assets/Game/Game.asmdef"),
        r#"{"name":"Game","references":["", "Shared"]}"#,
    );
    write(
        &root.join("Assets/Game/Game.asmdef.meta"),
        "fileFormatVersion: 2\nguid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
    );
    write(
        &root.join("Assets/Shared/Shared.asmdef"),
        r#"{"name":"Shared"}"#,
    );
    write(
        &root.join("Assets/Shared/Shared.asmdef.meta"),
        "fileFormatVersion: 2\nguid: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n",
    );
    write(&root.join("Assets/Broken.asmdef"), r#"{"name": }"#);
    write(&root.join("Assets/Refs/Ref.asmref"), r#"{"reference":""}"#);
    temp
}

fn file_url(path: &Path) -> Url {
    Url::from_file_path(path).unwrap()
}

#[test]
fn completes_defines_and_reports_malformed_files() {
    let temp = fixture();
    let root = temp.path();
    let game_path = root.join("Assets/Game/Game.asmdef");
    let game_url = file_url(&game_path);
    let broken_url = file_url(&root.join("Assets/Broken.asmdef"));
    let shared_path = root.join("Assets/Shared/Shared.asmdef");
    let shared_url = file_url(&shared_path);
    let ref_path = root.join("Assets/Refs/Ref.asmref");
    let ref_url = file_url(&ref_path);
    let ref_text = std::fs::read_to_string(&ref_path).unwrap();
    let game_text = std::fs::read_to_string(&game_path).unwrap();

    let mut client = LspClient::start();
    let result = client.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": file_url(root),
            "capabilities": {},
        }),
    );
    assert!(result["capabilities"]["completionProvider"].is_object());
    assert!(result["capabilities"]["textDocumentSync"]["save"].is_boolean());
    client.notify("initialized", json!({}));

    let diagnostics = client.wait_for_diagnostics(broken_url.as_str());
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "asmdef.parse-error"),
        "missing parse diagnostic: {diagnostics:?}"
    );

    client.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": game_url,
                "languageId": "json",
                "version": 1,
                "text": game_text,
            }
        }),
    );

    let empty_offset = game_text.find("\"\"").unwrap() + 1;
    let completion = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": game_url },
            "position": { "line": 0, "character": empty_offset },
        }),
    );
    let items = completion_items(&completion);
    assert!(
        items.iter().any(|item| item["label"] == "Shared"),
        "missing Shared completion: {items:?}"
    );

    let shared_offset = game_text.find("Shared").unwrap() + 2;
    let definition = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": game_url },
            "position": { "line": 0, "character": shared_offset },
        }),
    );
    let locations = definition_locations(&definition);
    assert!(
        locations.iter().any(|location| location["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("Shared.asmdef"))),
        "missing Shared definition: {locations:?}"
    );

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": shared_url, "version": 2 },
            "contentChanges": [{ "text": "{ \"name\": }" }],
        }),
    );
    let diagnostics = client.wait_for_diagnostics(shared_url.as_str());
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "asmdef.parse-error"),
        "missing Shared parse diagnostic: {diagnostics:?}"
    );

    let completion = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": game_url },
            "position": { "line": 0, "character": empty_offset },
        }),
    );
    let items = completion_items(&completion);
    assert!(
        items.iter().any(|item| item["label"] == "Shared"),
        "invalid open target must stay indexed: {items:?}"
    );

    client.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": ref_url,
                "languageId": "json",
                "version": 1,
                "text": ref_text,
            }
        }),
    );
    let ref_offset = ref_text.find("\"\"").unwrap() + 1;
    let completion = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": ref_url },
            "position": { "line": 0, "character": ref_offset },
        }),
    );
    let items = completion_items(&completion);
    assert!(
        items.iter().any(|item| item["label"] == "Shared"),
        "missing asmref completion: {items:?}"
    );
}

#[test]
fn check_mode_reports_broken_asmdefs() {
    let temp = fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_unity-asmdef-lsp"))
        .arg("--check")
        .arg(temp.path())
        .output()
        .expect("run --check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success(), "--check should fail: {stdout}");
    assert!(stdout.contains("asmdef.parse-error"), "{stdout}");
    assert!(stdout.contains("Broken.asmdef"), "{stdout}");
}
