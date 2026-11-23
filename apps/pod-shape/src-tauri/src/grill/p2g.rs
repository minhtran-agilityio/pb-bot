//! The p2g-questions protocol: find the fenced blocks in assistant text and validate them.
//! A port of `apps/pod-shape/spike/p2g.py` (same rules, same error texts), so the app checks exactly what
//! the spike proved. The schema is `spike/p2g-questions.schema.json` (the AskUserQuestion input shape).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub const TAG: &str = "p2g-questions";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opt {
    pub label: String,
    pub description: String,
    /// Not shown in step 3 (owner decision); kept so step 4 can show it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub question: String,
    pub header: String,
    pub multi_select: bool,
    pub options: Vec<Opt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub questions: Vec<Question>,
}

/// One fenced block: its info string and body.
#[derive(Debug, Clone, PartialEq)]
pub struct Fence {
    pub info: String,
    pub body: String,
    pub closed: bool,
}

/// `(blocks, outside)`: the fenced blocks and the text outside every fence (p2g.py `split_fences`).
pub fn split_fences(text: &str) -> (Vec<Fence>, String) {
    let mut blocks = Vec::new();
    let mut outside: Vec<&str> = Vec::new();
    let mut open: Option<(char, usize, String)> = None;
    let mut buf: Vec<&str> = Vec::new();
    for line in text.split('\n') {
        let fence = parse_fence(line);
        match &open {
            None => match fence {
                Some((ch, n, info)) => {
                    open = Some((ch, n, info));
                    buf.clear();
                }
                None => outside.push(line),
            },
            Some((ch, n, info)) => match fence {
                Some((c2, n2, i2)) if c2 == *ch && n2 >= *n && i2.is_empty() => {
                    blocks.push(Fence { info: info.clone(), body: buf.join("\n"), closed: true });
                    open = None;
                }
                _ => buf.push(line),
            },
        }
    }
    if let Some((_, _, info)) = open {
        blocks.push(Fence { info, body: buf.join("\n"), closed: false });
    }
    (blocks, outside.join("\n"))
}

/// `^[ \t]*(```+|~~~+)[ \t]*([^`]*?)[ \t]*$` -> (fence char, fence length, info).
fn parse_fence(line: &str) -> Option<(char, usize, String)> {
    let t = line.trim_start_matches([' ', '\t']);
    let ch = t.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let n = t.chars().take_while(|c| *c == ch).count();
    if n < 3 {
        return None;
    }
    let info = t[n..].trim_matches([' ', '\t']);
    if info.contains('`') {
        return None;
    }
    Some((ch, n, info.to_string()))
}

fn err(path: &str, msg: &str) -> String {
    format!("{path}: {msg}")
}

fn extra_keys(o: &serde_json::Map<String, Value>, allowed: &[&str]) -> Option<String> {
    let mut extra: Vec<&str> = o.keys().map(String::as_str).filter(|k| !allowed.contains(k)).collect();
    if extra.is_empty() {
        return None;
    }
    extra.sort_unstable();
    let list = extra.iter().map(|k| format!("'{k}'")).collect::<Vec<_>>().join(", ");
    Some(format!("unexpected keys [{list}]"))
}

fn non_empty_str(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::String(s)) if !s.is_empty())
}

/// Errors against the schema; empty means valid (p2g.py `validate`).
pub fn validate(obj: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(o) = obj.as_object() else {
        return vec![err("$", "not an object")];
    };
    if let Some(e) = extra_keys(o, &["questions"]) {
        errs.push(err("$", &e));
    }
    let qs = match o.get("questions") {
        Some(Value::Array(a)) if (1..=4).contains(&a.len()) => a,
        _ => {
            errs.push(err("$.questions", "must be an array of 1 to 4 items"));
            return errs;
        }
    };
    for (i, q) in qs.iter().enumerate() {
        let p = format!("$.questions[{i}]");
        let Some(q) = q.as_object() else {
            errs.push(err(&p, "not an object"));
            continue;
        };
        if let Some(e) = extra_keys(q, &["question", "header", "options", "multiSelect"]) {
            errs.push(err(&p, &e));
        }
        for key in ["question", "header"] {
            if !non_empty_str(q.get(key)) {
                errs.push(err(&format!("{p}.{key}"), "must be a non-empty string"));
            }
        }
        if let Some(Value::String(h)) = q.get("header") {
            let n = h.chars().count();
            if n > 12 {
                errs.push(err(&format!("{p}.header"), &format!("longer than 12 characters ({n})")));
            }
        }
        if !matches!(q.get("multiSelect"), Some(Value::Bool(_))) {
            errs.push(err(&format!("{p}.multiSelect"), "must be a boolean"));
        }
        let opts = match q.get("options") {
            Some(Value::Array(a)) if (2..=4).contains(&a.len()) => a,
            _ => {
                errs.push(err(&format!("{p}.options"), "must be an array of 2 to 4 items"));
                continue;
            }
        };
        let mut labels = Vec::new();
        for (j, opt) in opts.iter().enumerate() {
            let op = format!("{p}.options[{j}]");
            let Some(opt) = opt.as_object() else {
                errs.push(err(&op, "not an object"));
                continue;
            };
            if let Some(e) = extra_keys(opt, &["label", "description", "preview"]) {
                errs.push(err(&op, &e));
            }
            if !non_empty_str(opt.get("label")) {
                errs.push(err(&format!("{op}.label"), "must be a non-empty string"));
            }
            if !matches!(opt.get("description"), Some(Value::String(_))) {
                errs.push(err(&format!("{op}.description"), "must be a string"));
            }
            if opt.get("preview").is_some_and(|v| !v.is_string()) {
                errs.push(err(&format!("{op}.preview"), "must be a string"));
            }
            if let Some(Value::String(l)) = opt.get("label") {
                labels.push(l.as_str());
            }
        }
        if labels.len() != labels.iter().collect::<HashSet<_>>().len() {
            errs.push(err(&format!("{p}.options"), "duplicate labels"));
        }
    }
    // Answers are keyed by question text (protocol rule 6), so two equal texts lose an answer.
    let texts: Vec<&str> = qs.iter().filter_map(|q| q.get("question").and_then(Value::as_str)).collect();
    if texts.len() != texts.iter().collect::<HashSet<_>>().len() {
        errs.push(err("$.questions", "duplicate question texts"));
    }
    errs
}

/// One p2g-questions block found in a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub errors: Vec<String>,
    /// Set only when the block is valid.
    pub block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub blocks: Vec<Found>,
    /// Prose lines that end in "?" (a question outside a block; heuristic, as in the spike).
    pub leaks: Vec<String>,
    /// The message text with every p2g-questions block cut out, for the chat.
    pub prose: String,
}

/// Checks one assistant message (p2g.py `analyse`).
pub fn analyse(text: &str) -> Analysis {
    let (fences, outside) = split_fences(text);
    let mut blocks = Vec::new();
    for f in fences.iter().filter(|f| f.info.contains(TAG)) {
        let mut errors = Vec::new();
        if !f.closed {
            errors.push("unclosed fence".to_string());
        }
        let mut block = None;
        match serde_json::from_str::<Value>(&f.body) {
            Err(e) => errors.push(format!("invalid JSON: {e}")),
            Ok(v) => {
                errors.extend(validate(&v));
                if errors.is_empty() {
                    block = serde_json::from_value::<Block>(v).ok();
                }
            }
        }
        blocks.push(Found { errors, block });
    }
    let mut leaks: Vec<String> = outside
        .split('\n')
        .map(str::trim)
        .filter(|l| l.trim_end_matches(['*', '_', ' ', ')']).ends_with('?') && !l.starts_with(['|', '>']))
        .map(str::to_string)
        .collect();
    if outside.contains(TAG) {
        leaks.push(format!("'{TAG}' named outside a fenced block"));
    }
    Analysis { blocks, leaks, prose: strip_blocks(text) }
}

/// The text without its p2g-questions blocks (other fences, such as code, stay).
pub fn strip_blocks(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut open: Option<(char, usize, bool)> = None; // (char, len, is_p2g)
    for line in text.split('\n') {
        let fence = parse_fence(line);
        match open {
            None => match fence {
                Some((ch, n, info)) => {
                    let p2g = info.contains(TAG);
                    open = Some((ch, n, p2g));
                    if !p2g {
                        out.push(line);
                    }
                }
                None => out.push(line),
            },
            Some((ch, n, p2g)) => {
                let closes = matches!(&fence, Some((c2, n2, i2)) if *c2 == ch && *n2 >= n && i2.is_empty());
                if !p2g {
                    out.push(line);
                }
                if closes {
                    open = None;
                }
            }
        }
    }
    out.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn good() -> Value {
        json!({"questions": [{"question": "Pick one?", "header": "scope", "multiSelect": false,
            "options": [{"label": "A", "description": "a"}, {"label": "B", "description": "b"}]}]})
    }

    // The 7 cases of p2g.py `_selftest`.
    #[test]
    fn a_valid_block_with_intro_text_has_no_errors_and_no_leaks() {
        let msg = format!("Intro text.\n\n```p2g-questions\n{}\n```\n", good());
        let r = analyse(&msg);
        assert_eq!(r.blocks.len(), 1);
        assert!(r.blocks[0].errors.is_empty());
        assert!(r.blocks[0].block.is_some());
        assert!(r.leaks.is_empty());
        assert_eq!(r.prose, "Intro text.");
    }

    #[test]
    fn a_long_header_and_no_options_give_two_errors() {
        let mut bad = good();
        bad["questions"][0]["header"] = json!("far too long a header");
        bad["questions"][0]["options"] = json!([]);
        let r = analyse(&format!("```p2g-questions\n{bad}\n```"));
        assert_eq!(r.blocks[0].errors.len(), 2, "{:?}", r.blocks[0].errors);
        assert!(r.blocks[0].block.is_none());
    }

    #[test]
    fn invalid_json_is_reported() {
        let r = analyse("```p2g-questions\n{not json}\n```");
        assert!(r.blocks[0].errors[0].starts_with("invalid JSON"));
    }

    #[test]
    fn a_question_in_prose_is_a_leak() {
        let r = analyse("Does this brief say what you mean?\n");
        assert_eq!(r.leaks, vec!["Does this brief say what you mean?"]);
        assert!(r.blocks.is_empty());
    }

    #[test]
    fn other_fences_are_not_blocks_and_their_question_marks_are_not_leaks() {
        let r = analyse("```json\n{\"a\": \"why?\"}\n```\nDone.");
        assert!(r.blocks.is_empty());
        assert!(r.leaks.is_empty());
    }

    #[test]
    fn an_unclosed_fence_is_an_error() {
        let r = analyse(&format!("```p2g-questions\n{}", good()));
        assert_eq!(r.blocks[0].errors, vec!["unclosed fence"]);
    }

    #[test]
    fn duplicate_labels_and_question_texts_are_errors() {
        let q = good()["questions"][0].clone();
        let mut q2 = q.clone();
        q2["options"] = json!([q["options"][0], q["options"][0]]);
        let errs = validate(&json!({"questions": [q, q2]}));
        assert_eq!(errs, vec!["$.questions[1].options: duplicate labels", "$.questions: duplicate question texts"]);
    }

    // Beyond the selftest.
    #[test]
    fn extra_keys_and_wrong_types_are_errors() {
        let errs = validate(&json!({"questions": [{"question": "Q?", "header": "h", "multiSelect": "no",
            "options": [{"label": "A", "description": 1, "x": 1}, {"label": "", "description": "b", "preview": 2}]}],
            "extra": true}));
        assert!(errs.contains(&"$: unexpected keys ['extra']".to_string()), "{errs:?}");
        assert!(errs.contains(&"$.questions[0].multiSelect: must be a boolean".to_string()));
        assert!(errs.contains(&"$.questions[0].options[0]: unexpected keys ['x']".to_string()));
        assert!(errs.contains(&"$.questions[0].options[0].description: must be a string".to_string()));
        assert!(errs.contains(&"$.questions[0].options[1].label: must be a non-empty string".to_string()));
        assert!(errs.contains(&"$.questions[0].options[1].preview: must be a string".to_string()));
    }

    #[test]
    fn a_header_of_12_characters_is_allowed_and_counted_in_characters() {
        let mut v = good();
        v["questions"][0]["header"] = json!("Phát hành 12");
        assert!(validate(&v).is_empty());
        v["questions"][0]["header"] = json!("1234567890123");
        assert_eq!(validate(&v), vec!["$.questions[0].header: longer than 12 characters (13)"]);
    }

    #[test]
    fn five_questions_are_too_many() {
        let q = good()["questions"][0].clone();
        let qs: Vec<Value> = (0..5).map(|i| { let mut x = q.clone(); x["question"] = json!(format!("Q{i}?")); x }).collect();
        assert_eq!(validate(&json!({"questions": qs})), vec!["$.questions: must be an array of 1 to 4 items"]);
    }

    #[test]
    fn a_tilde_fence_and_a_preview_parse() {
        let mut v = good();
        v["questions"][0]["options"][0]["preview"] = json!("| a |");
        v["questions"][0]["multiSelect"] = json!(true);
        let r = analyse(&format!("Text\n~~~p2g-questions\n{v}\n~~~\nafter"));
        let b = r.blocks[0].block.clone().unwrap();
        assert!(b.questions[0].multi_select);
        assert_eq!(b.questions[0].options[0].preview.as_deref(), Some("| a |"));
        assert_eq!(r.prose, "Text\nafter");
    }

    #[test]
    fn strip_blocks_keeps_code_fences() {
        let t = format!("A\n```rust\nfn x() {{}}\n```\n```p2g-questions\n{}\n```", good());
        assert_eq!(strip_blocks(&t), "A\n```rust\nfn x() {}\n```");
    }
}
