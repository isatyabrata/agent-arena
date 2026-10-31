//! labctl: small client so any harness can drive the lab from a shell.
//!
//!   labctl join   --id e1 --name "Ada" --role engineer --model claude-opus --harness claude-code --bio "..."
//!   labctl status --id e1 --activity typing --task "writing loader" --screen "main.rs|fn load()"
//!   labctl output --id e1 --text "cargo test: 12 passed"
//!   some-agent-cli | labctl tee --id e1     (mirror a whole terminal into the lab, still prints it)
//!   labctl hook   --id e1                   (Claude Code hook: reads the hook JSON from stdin)
//!   labctl goto   --id e1 --place whiteboard
//!   labctl say    --id e1 --text "found a bug"
//!   labctl note   --by s1 --kind works --text "sparse memory helps"
//!   labctl flag   --by auditor --target r2 --reason "renamed known architecture"
//!   labctl leave  --id e1
//!   labctl raw    '{"type":"log","text":"hello"}'   (any event, see docs/PROTOCOL.md)
//!   cat events.jsonl | labctl pipe          (one JSON event per line)
//!   labctl config                           (print the live config as TOML)
//!   labctl config-set lab.toml              (replace the live config; also saved on the server)
//!   labctl setup-claude --dir ~/proj --id e1 --name Ada --role engineer --model claude-opus
//!                                           (wire a Claude Code project's hooks to the lab)
//!
//! Server URL: --url or LAB_URL (default http://127.0.0.1:7878). Token: LAB_TOKEN.
//! --id defaults to LAB_AGENT. Exit code is 0 or 1, never 2, so hooks never block.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn request(url: &str, method: &str, path: &str, body: &str) -> Result<String, String> {
    let rest = url.strip_prefix("http://").ok_or("only http:// URLs are supported")?;
    let host = rest.split('/').next().unwrap_or(rest);
    let mut s = TcpStream::connect(host).map_err(|e| format!("connect {host}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let auth = std::env::var("LAB_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    s.read_to_string(&mut resp).map_err(|e| e.to_string())?;
    let ok = resp.starts_with("HTTP/1.1 200");
    let body = resp.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("").to_string();
    if ok {
        Ok(body)
    } else {
        Err(body)
    }
}

fn post(url: &str, v: &Value) -> Result<String, String> {
    request(url, "POST", "/api/events", &v.to_string())
}

fn short(s: &str, n: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}...", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

/// Translate one Claude Code hook payload into lab events.
fn hook_events(id: &str, h: &Value, kv: &HashMap<String, String>) -> Vec<Value> {
    let ev = h["hook_event_name"].as_str().unwrap_or("");
    let tool = h["tool_name"].as_str().unwrap_or("");
    let input = &h["tool_input"];
    let target = input["command"]
        .as_str()
        .or(input["file_path"].as_str())
        .or(input["pattern"].as_str())
        .or(input["url"].as_str())
        .or(input["description"].as_str())
        .unwrap_or("");
    let status = |activity: &str, task: String| json!({"type":"status","id":id,"activity":activity,"task":task,"screen":[tool, short(target, 60)]});
    let out = |text: String| json!({"type":"output","id":id,"text":text});
    match ev {
        "SessionStart" => {
            let name = kv.get("name").cloned().or_else(|| std::env::var("LAB_NAME").ok()).unwrap_or_else(|| id.to_string());
            let role = kv.get("role").cloned().or_else(|| std::env::var("LAB_ROLE").ok()).unwrap_or_else(|| "engineer".into());
            let model = kv.get("model").cloned().or_else(|| std::env::var("LAB_MODEL").ok()).unwrap_or_else(|| "claude".into());
            vec![
                json!({"type":"join","id":id,"name":name,"role":role,"model":model,"harness":"claude-code"}),
                out(format!("--- session {} started ---", h["session_id"].as_str().unwrap_or(""))),
            ]
        }
        "UserPromptSubmit" => {
            let p = h["prompt"].as_str().unwrap_or("");
            vec![status("thinking", format!("prompt: {}", short(p, 50))), out(format!("> {}", p))]
        }
        "PreToolUse" => {
            let activity = match tool {
                "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "typing",
                "Read" | "Grep" | "Glob" | "WebFetch" | "WebSearch" | "LS" => "reading",
                "Bash" => "testing",
                "Task" | "Agent" => "talking",
                _ => "thinking",
            };
            let line = match tool {
                "Bash" => format!("$ {}", input["command"].as_str().unwrap_or("")),
                _ => format!("[{}] {}", tool, short(target, 200)),
            };
            vec![status(activity, format!("{} {}", tool, short(target, 40))), out(line)]
        }
        "PostToolUse" => {
            let r = &h["tool_response"];
            let text = r["stdout"]
                .as_str()
                .map(str::to_string)
                .or_else(|| r.as_str().map(str::to_string))
                .or_else(|| r["output"].as_str().map(str::to_string))
                .unwrap_or_default();
            let mut lines: Vec<&str> = text.lines().collect();
            let more = lines.len().saturating_sub(30);
            lines.truncate(30);
            let mut body = lines.join("\n");
            if more > 0 {
                body.push_str(&format!("\n... ({more} more lines)"));
            }
            if let Some(e) = r["stderr"].as_str().filter(|e| !e.is_empty()) {
                body.push_str(&format!("\n{}", short(e, 300)));
            }
            if body.is_empty() {
                vec![]
            } else {
                vec![out(body)]
            }
        }
        "Stop" | "SubagentStop" => vec![status("waiting", "waiting for input".into()), out("--- turn finished ---".into())],
        "Notification" => {
            let m = h["message"].as_str().unwrap_or("needs attention");
            vec![json!({"type":"say","id":id,"text":short(m, 34)}), out(format!("! {m}"))]
        }
        "SessionEnd" => vec![out("--- session ended ---".into()), json!({"type":"status","id":id,"activity":"idle","task":"offline"})],
        _ => vec![],
    }
}

/// Merge lab hooks and env into <dir>/.claude/settings.local.json (kept out of git by Claude Code).
fn setup_claude(kv: &HashMap<String, String>, url: &str, id: &str) -> Result<String, String> {
    let dir = kv.get("dir").cloned().unwrap_or_else(|| ".".into());
    if id.is_empty() {
        return Err("--id is required".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let cmd = format!("\"{}\" hook", exe.display());
    let path = std::path::Path::new(&dir).join(".claude").join("settings.local.json");
    let mut root: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    if !root.is_object() {
        root = json!({});
    }
    let env = root.as_object_mut().unwrap().entry("env").or_insert(json!({}));
    let pairs = [
        ("LAB_AGENT", Some(id.to_string())),
        ("LAB_NAME", kv.get("name").cloned()),
        ("LAB_ROLE", kv.get("role").cloned()),
        ("LAB_MODEL", kv.get("model").cloned()),
        ("LAB_URL", Some(url.to_string())),
        ("LAB_TOKEN", std::env::var("LAB_TOKEN").ok().filter(|t| !t.is_empty())),
    ];
    for (k, v) in pairs {
        if let Some(v) = v {
            env[k] = json!(v);
        }
    }
    let hooks = root.as_object_mut().unwrap().entry("hooks").or_insert(json!({}));
    for ev in ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Notification", "Stop", "SessionEnd"] {
        let list = hooks.as_object_mut().unwrap().entry(ev).or_insert(json!([]));
        if !list.is_array() {
            *list = json!([]);
        }
        let arr = list.as_array_mut().unwrap();
        // drop any older labctl entry, then add ours
        arr.retain(|g| !g.to_string().contains("labctl"));
        let mut group = json!({"hooks":[{"type":"command","command":cmd}]});
        if ev == "PreToolUse" || ev == "PostToolUse" {
            group["matcher"] = json!("*");
        }
        arr.push(group);
    }
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(&root).unwrap()).map_err(|e| e.to_string())?;
    // announce the agent right away so it appears before its next session starts
    let _ = post(url, &json!({"type":"join","id":id,"name":kv.get("name").cloned().unwrap_or(id.to_string()),
        "role":kv.get("role").cloned().unwrap_or("engineer".into()),"model":kv.get("model"),"harness":"claude-code"}));
    Ok(format!("wrote {} (restart Claude Code in that folder to pick it up)", path.display()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: labctl <join|leave|status|output|tee|hook|goto|say|note|flag|phase|log|raw|pipe|config|config-set|setup-claude> [--key value ...]");
        std::process::exit(1);
    }
    let cmd = args[0].as_str();
    let mut kv: HashMap<String, String> = HashMap::new();
    let mut positional = vec![];
    let mut i = 1;
    while i < args.len() {
        if let Some(k) = args[i].strip_prefix("--") {
            let v = args.get(i + 1).filter(|v| !v.starts_with("--")).cloned();
            i += if v.is_some() { 2 } else { 1 };
            kv.insert(k.to_string(), v.unwrap_or_default());
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }
    let url = kv.get("url").cloned().or_else(|| std::env::var("LAB_URL").ok()).unwrap_or_else(|| "http://127.0.0.1:7878".into());
    let id = kv.get("id").cloned().or_else(|| std::env::var("LAB_AGENT").ok()).unwrap_or_default();
    let g = |k: &str| kv.get(k).cloned().unwrap_or_default();
    let opt = |k: &str| kv.get(k).cloned();

    let result = match cmd {
        "join" => post(
            &url,
            &json!({"type":"join","id":id,"name":opt("name").unwrap_or(id.clone()),"role":opt("role").unwrap_or("guest".into()),
                    "model":opt("model"),"harness":opt("harness"),"bio":opt("bio"),"color":opt("color")}),
        ),
        "leave" => post(&url, &json!({"type":"leave","id":id})),
        "goto" => post(&url, &json!({"type":"goto","id":id,"place":g("place")})),
        "say" => post(&url, &json!({"type":"say","id":id,"text":g("text")})),
        "output" => post(&url, &json!({"type":"output","id":id,"text":g("text")})),
        "status" => {
            let mut v = json!({"type":"status","id":id,"activity":opt("activity").unwrap_or("typing".into())});
            if let Some(t) = opt("task") {
                v["task"] = json!(t);
            }
            if let Some(s) = opt("screen") {
                v["screen"] = json!(s.split('|').collect::<Vec<_>>());
            }
            post(&url, &v)
        }
        "note" => post(&url, &json!({"type":"note","kind":opt("kind").unwrap_or("info".into()),"text":g("text"),"by":opt("by").unwrap_or(id.clone())})),
        "flag" => post(&url, &json!({"type":"flag","by":opt("by").unwrap_or(id.clone()),"target":g("target"),"reason":g("reason")})),
        "phase" => post(&url, &json!({"type":"phase","name":g("name")})),
        "log" => post(&url, &json!({"type":"log","text":g("text")})),
        "raw" => request(&url, "POST", "/api/events", &positional.join(" ")),
        "pipe" => {
            let lines: Vec<String> = std::io::stdin().lock().lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).collect();
            request(&url, "POST", "/api/events", &format!("[{}]", lines.join(",")))
        }
        "tee" => {
            // stream stdin into the agent's terminal while still printing it
            let quiet = kv.contains_key("quiet");
            let mut buf: Vec<String> = vec![];
            let mut last = Instant::now();
            let flush = |buf: &mut Vec<String>| {
                if !buf.is_empty() {
                    let _ = post(&url, &json!({"type":"output","id":id,"text":buf.join("\n")}));
                    buf.clear();
                }
            };
            for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                if !quiet {
                    println!("{line}");
                }
                buf.push(line);
                if buf.len() >= 40 || last.elapsed() > Duration::from_millis(300) {
                    flush(&mut buf);
                    last = Instant::now();
                }
            }
            flush(&mut buf);
            Ok(String::new())
        }
        "hook" => {
            let mut s = String::new();
            let _ = std::io::stdin().read_to_string(&mut s);
            let h: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
            let evs = hook_events(&id, &h, &kv);
            if evs.is_empty() {
                Ok(String::new())
            } else {
                // a hook must never break the harness: ignore network errors
                let _ = post(&url, &Value::Array(evs));
                Ok(String::new())
            }
        }
        "config" => request(&url, "GET", "/api/config", ""),
        "setup-claude" => setup_claude(&kv, &url, &id),
        "config-set" => match positional.first().map(std::fs::read_to_string) {
            Some(Ok(text)) => request(&url, "POST", "/api/config", &text),
            Some(Err(e)) => Err(e.to_string()),
            None => Err("usage: labctl config-set <file.toml>".into()),
        },
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(r) => {
            if !r.is_empty() && cmd != "tee" {
                println!("{r}");
            }
        }
        Err(e) => {
            eprintln!("labctl: {e}");
            std::process::exit(1);
        }
    }
}
