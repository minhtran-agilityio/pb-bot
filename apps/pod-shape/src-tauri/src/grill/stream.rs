//! `claude -p --output-format stream-json --verbose`: one JSON event per line. Only the events the app
//! uses are kept (advisor should-fix: a 267 KB prototype read must not land in SQLite as a tool result).

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Init { session_id: String, permission_mode: String },
    /// Top-level assistant text (subagent text has a `parent_tool_use_id` and is skipped).
    Text(String),
    /// A tool call: name, a short summary for the history, and the file path for Write/Edit.
    ToolUse { name: String, summary: String, file_path: Option<String> },
    Result(TurnResult),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TurnResult {
    pub subtype: String,
    pub is_error: bool,
    /// The final text, or the error text.
    pub result: String,
    /// Cumulative over --resume (spike RESULTS: the session cost is the last turn's value).
    pub total_cost_usd: Option<f64>,
    /// Tool name and a short input summary of each denied call.
    pub denials: Vec<String>,
}

impl TurnResult {
    pub fn ok(&self) -> bool {
        !self.is_error && self.subtype == "success"
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// A short, single-line summary of a tool input: the path, pattern or command, never the content.
pub fn tool_summary(name: &str, input: &Value) -> String {
    let pick = |k: &str| input.get(k).and_then(Value::as_str).map(str::to_string);
    let raw = match name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => pick("file_path"),
        "Glob" => pick("pattern"),
        "Grep" => pick("pattern").map(|p| match pick("path") {
            Some(path) => format!("{p} in {path}"),
            None => p,
        }),
        "Bash" => pick("command"),
        "Skill" => pick("skill"),
        _ => None,
    }
    .unwrap_or_default();
    let one: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 160 {
        one.chars().take(159).collect::<String>() + "…"
    } else {
        one
    }
}

/// Parses one line. `None` for a line that is not JSON or not a used event.
pub fn parse_line(line: &str) -> Vec<Event> {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
        return vec![];
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
            vec![Event::Init { session_id: s(&v, "session_id"), permission_mode: s(&v, "permissionMode") }]
        }
        Some("assistant") => {
            if v.get("parent_tool_use_id").is_some_and(|p| !p.is_null()) {
                return vec![];
            }
            let mut out = vec![];
            let content = v.pointer("/message/content").and_then(Value::as_array).cloned().unwrap_or_default();
            for b in content {
                match b.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let t = s(&b, "text");
                        if !t.trim().is_empty() {
                            out.push(Event::Text(t));
                        }
                    }
                    Some("tool_use") => {
                        let name = s(&b, "name");
                        let input = b.get("input").cloned().unwrap_or(Value::Null);
                        let file_path = match name.as_str() {
                            "Write" | "Edit" => input.get("file_path").and_then(Value::as_str).map(str::to_string),
                            _ => None,
                        };
                        out.push(Event::ToolUse { summary: tool_summary(&name, &input), name, file_path });
                    }
                    _ => {}
                }
            }
            out
        }
        Some("result") => {
            let denials = v
                .get("permission_denials")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|d| {
                            let name = s(d, "tool_name");
                            let input = d.get("tool_input").cloned().unwrap_or(Value::Null);
                            format!("{name} {}", tool_summary(&name, &input)).trim().to_string()
                        })
                        .collect()
                })
                .unwrap_or_default();
            vec![Event::Result(TurnResult {
                subtype: s(&v, "subtype"),
                is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                result: s(&v, "result"),
                total_cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                denials,
            })]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_gives_the_session_and_the_permission_mode() {
        let e = parse_line(r#"{"type":"system","subtype":"init","session_id":"abc","permissionMode":"dontAsk","tools":[]}"#);
        assert_eq!(e, vec![Event::Init { session_id: "abc".into(), permission_mode: "dontAsk".into() }]);
    }

    #[test]
    fn assistant_text_and_tool_use_are_split_and_subagent_text_is_skipped() {
        let line = r##"{"type":"assistant","parent_tool_use_id":null,"message":{"content":[
            {"type":"text","text":"Reading it."},
            {"type":"tool_use","name":"Write","input":{"file_path":"/r/specs/x/brief.md","content":"# Brief: X\n..."}},
            {"type":"tool_use","name":"Read","input":{"file_path":"/tmp/p/prototype.html"}}]}}"##;
        let e = parse_line(line);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0], Event::Text("Reading it.".into()));
        assert_eq!(
            e[1],
            Event::ToolUse { name: "Write".into(), summary: "/r/specs/x/brief.md".into(), file_path: Some("/r/specs/x/brief.md".into()) }
        );
        assert!(matches!(&e[2], Event::ToolUse { name, file_path: None, .. } if name == "Read"));
        let sub = r#"{"type":"assistant","parent_tool_use_id":"t1","message":{"content":[{"type":"text","text":"x"}]}}"#;
        assert!(parse_line(sub).is_empty());
    }

    #[test]
    fn a_tool_result_line_is_not_kept() {
        let big = format!(r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"{}"}}]}}}}"#, "x".repeat(10_000));
        assert!(parse_line(&big).is_empty());
        assert!(parse_line("not json").is_empty());
    }

    #[test]
    fn a_success_result_and_its_denials() {
        let e = parse_line(r#"{"type":"result","subtype":"success","is_error":false,"result":"Done","total_cost_usd":0.153,
            "permission_denials":[{"tool_name":"Read","tool_input":{"file_path":"/home/u/.ssh/id_rsa"}}]}"#);
        let Event::Result(r) = &e[0] else { panic!() };
        assert!(r.ok());
        assert_eq!(r.total_cost_usd, Some(0.153));
        assert_eq!(r.denials, vec!["Read /home/u/.ssh/id_rsa"]);
    }

    #[test]
    fn a_usage_limit_result_is_an_error_with_its_real_text() {
        let e = parse_line(r#"{"type":"result","subtype":"success","is_error":true,"result":"Claude AI usage limit reached|1764000000"}"#);
        let Event::Result(r) = &e[0] else { panic!() };
        assert!(!r.ok());
        assert_eq!(r.result, "Claude AI usage limit reached|1764000000");
        let e = parse_line(r#"{"type":"result","subtype":"error_during_execution","is_error":false,"result":""}"#);
        let Event::Result(r) = &e[0] else { panic!() };
        assert!(!r.ok());
    }

    #[test]
    fn summaries_are_one_short_line() {
        let v = serde_json::json!({"command": "git   status\n --short"});
        assert_eq!(tool_summary("Bash", &v), "git status --short");
        let long = serde_json::json!({"pattern": "x".repeat(400)});
        assert_eq!(tool_summary("Glob", &long).chars().count(), 160);
    }
}
