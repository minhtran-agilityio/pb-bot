//! The `claude -p` command line of one grill turn: the flag set of `spike/README.md`, ported, with the
//! read rules narrowed and the ADR git work moved to the app (specs design §3, advisor must-fix 1 and 5).
//!
//! Probes in this build (haiku, `--permission-mode dontAsk`):
//! - a bare `Read` reads any file, so the read tools are scoped to the repo, the prototype folder and
//!   the plugin folder; `Read(//<abs>/**)` is the absolute form;
//! - a `*` inside a Bash rule also matches extra arguments, so no Bash rule is given to Claude at all.

use std::path::Path;

/// The spike's protocol text, verbatim (one source of truth).
pub const SPIKE_PROMPT: &str = include_str!("../../../spike/append-system-prompt.md");

/// Model and effort of the spike runs.
pub const MODEL: &str = "sonnet";
pub const EFFORT: &str = "medium";

/// Exact headers the app recognizes (rule 8 below).
pub const HEADER_BRIEF: &str = "Brief";
pub const HEADER_ADR: &str = "ADR";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Every grill turn.
    Grill,
    /// The one turn after the owner confirmed an ADR in the app dialog: Claude may also write the
    /// decisions file. The app runs write_adrs.py, git and gh itself (adr.rs).
    Adr,
}

impl Step {
    pub fn as_str(self) -> &'static str {
        match self {
            Step::Grill => "grill",
            Step::Adr => "adr",
        }
    }
}

/// The app lines after the spike text. They are the same on every turn of a session (the system prompt
/// is recorded on the first turn; spike README), so they go on every session from turn 1.
pub fn app_prompt(decisions_file: &Path) -> String {
    format!(
        "{spike}\n\
         8. Set \"header\" to exactly \"{HEADER_BRIEF}\" on the question that asks whether the brief says what the owner means, \
         and to exactly \"{HEADER_ADR}\" on a question that asks whether to make a decision an ADR. Do not use these two headers for any other question.\n\
         9. ADRs: the app does all git and GitHub work, and you have no shell. When the owner answers yes to an \"{HEADER_ADR}\" question, \
         do not run git or gh and do not write under docs/adr/. Write the decisions file ({{\"run\": \"<slug>\", \"decisions\": [...]}}, \
         the fields of pd-decision-record \"How an agent returns it\", with stage \"grilling\", adr true, owner_confirmed true, decided_by \"owner\") \
         to {file}. Then go on with the grill. The app writes the ADR on branch adr/<slug>, pushes only that branch, opens a pull request, \
         and tells you the result in a later message.\n",
        spike = SPIKE_PROMPT.trim_end(),
        file = decisions_file.display()
    )
}

/// An absolute path rule: `//abs/path/**` (a single leading `/` would be relative to the settings file).
fn abs_rule(tool: &str, dir: &Path) -> String {
    format!("{tool}(/{}/**)", dir.display().to_string().trim_end_matches('/'))
}

pub fn allowed_tools(step: Step, proto_dir: &Path, decisions_dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = ["Read(./**)", "Glob(./**)", "Grep(./**)"].iter().map(|s| s.to_string()).collect();
    v.push(abs_rule("Read", proto_dir));
    // pd-shape reads its own references/*.md from the installed plugin.
    v.push("Read(~/.claude/plugins/**)".into());
    v.push("Write(./specs/**)".into());
    v.push("Edit(./specs/**)".into());
    if step == Step::Adr {
        v.push(abs_rule("Read", decisions_dir));
        v.push(abs_rule("Write", decisions_dir));
        v.push(abs_rule("Edit", decisions_dir));
    }
    v
}

pub fn disallowed_tools() -> Vec<String> {
    [
        // The spike's list: no owner dialog, no shell, no subagents, no web, no notebooks.
        "AskUserQuestion",
        "Bash",
        "Agent",
        "WebFetch",
        "WebSearch",
        "NotebookEdit",
        // Secrets in the repo (deny beats allow; Grep and Glob path rules are best effort).
        "Read(./.env*)",
        "Read(./**/.env*)",
        "Grep(./.env*)",
        "Grep(./**/.env*)",
        "Glob(./.env*)",
        "Glob(./**/.env*)",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// What one turn needs to build its command line.
pub struct TurnSpec<'a> {
    pub session_uuid: &'a str,
    /// `--session-id` (first turn) or `--resume`.
    pub first: bool,
    pub step: Step,
    pub proto_dir: &'a Path,
    pub decisions_dir: &'a Path,
    pub decisions_file: &'a Path,
    pub message: &'a str,
}

/// The arguments after `claude`. The message comes after `--`, so an answer that starts with `-` is
/// never read as a flag.
pub fn turn_args(t: &TurnSpec) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-p".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--strict-mcp-config".into(),
        // Pinned: the user's or the repo's defaultMode (auto in this checkout) must not decide.
        "--permission-mode".into(),
        "dontAsk".into(),
        "--permission-prompts".into(),
        "none".into(),
        "--model".into(),
        MODEL.into(),
        "--effort".into(),
        EFFORT.into(),
        // Every turn: after a compaction the prompt is rendered again from this launch's flags.
        "--append-system-prompt".into(),
        app_prompt(t.decisions_file),
        "--add-dir".into(),
        t.proto_dir.display().to_string(),
    ];
    a.push(if t.first { "--session-id" } else { "--resume" }.into());
    a.push(t.session_uuid.into());
    a.push("--allowedTools".into());
    a.extend(allowed_tools(t.step, t.proto_dir, t.decisions_dir));
    a.push("--disallowedTools".into());
    a.extend(disallowed_tools());
    a.push("--".into());
    a.push(t.message.into());
    a
}

/// One line for the RunLog: `claude -p /pd-bot:pd-shape … --session-id 1a2b3c4d`.
pub fn display_command(t: &TurnSpec) -> String {
    let short = &t.session_uuid[..t.session_uuid.len().min(8)];
    let what = if t.message.starts_with('/') {
        t.message.split_whitespace().next().unwrap_or("").to_string()
    } else {
        "(answers)".to_string()
    };
    let flag = if t.first { "--session-id" } else { "--resume" };
    let step = if t.step == Step::Adr { " (ADR turn)" } else { "" };
    format!("claude -p {what} {flag} {short}{step}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec<'a>(first: bool, step: Step, p: &'a Path, d: &'a Path, f: &'a Path, msg: &'a str) -> TurnSpec<'a> {
        TurnSpec { session_uuid: "11111111-2222-3333-4444-555555555555", first, step, proto_dir: p, decisions_dir: d, decisions_file: f, message: msg }
    }

    fn after<'a>(a: &'a [String], flag: &str) -> &'a str {
        let i = a.iter().position(|x| x == flag).unwrap_or_else(|| panic!("{flag} missing"));
        &a[i + 1]
    }

    fn list<'a>(a: &'a [String], flag: &str) -> Vec<&'a str> {
        let i = a.iter().position(|x| x == flag).unwrap() + 1;
        a[i..].iter().take_while(|x| !x.starts_with("--")).map(String::as_str).collect()
    }

    fn dirs() -> (PathBuf, PathBuf, PathBuf) {
        (PathBuf::from("/private/var/T/shape-board/u/proto"), PathBuf::from("/private/var/T/shape-board/u/decisions"),
         PathBuf::from("/private/var/T/shape-board/u/decisions/decisions.json"))
    }

    #[test]
    fn every_step_pins_dont_ask_and_never_skips_permissions() {
        let (p, d, f) = dirs();
        for step in [Step::Grill, Step::Adr] {
            for first in [true, false] {
                let a = turn_args(&spec(first, step, &p, &d, &f, "x"));
                assert_eq!(after(&a, "--permission-mode"), "dontAsk");
                assert_eq!(after(&a, "--permission-prompts"), "none");
                assert_eq!(after(&a, "--output-format"), "stream-json");
                assert!(a.contains(&"--verbose".to_string()));
                assert!(a.contains(&"--strict-mcp-config".to_string()));
                assert_eq!(after(&a, "--model"), "sonnet");
                assert_eq!(after(&a, "--effort"), "medium");
                assert!(!a.iter().any(|x| x.contains("dangerously")), "{a:?}");
                assert!(!a.iter().any(|x| x == "bypassPermissions" || x == "auto" || x == "acceptEdits"));
                // The append prompt rides on every turn and holds the spike text verbatim.
                assert!(after(&a, "--append-system-prompt").starts_with(SPIKE_PROMPT.trim_end()));
            }
        }
    }

    #[test]
    fn the_first_turn_uses_session_id_and_later_turns_resume() {
        let (p, d, f) = dirs();
        let a = turn_args(&spec(true, Step::Grill, &p, &d, &f, "/pd-bot:pd-shape x"));
        assert_eq!(after(&a, "--session-id"), "11111111-2222-3333-4444-555555555555");
        assert!(!a.contains(&"--resume".to_string()));
        let b = turn_args(&spec(false, Step::Grill, &p, &d, &f, "{}"));
        assert_eq!(after(&b, "--resume"), "11111111-2222-3333-4444-555555555555");
        assert!(!b.contains(&"--session-id".to_string()));
    }

    #[test]
    fn the_message_comes_last_after_a_double_dash() {
        let (p, d, f) = dirs();
        let a = turn_args(&spec(false, Step::Grill, &p, &d, &f, "--dangerous-looking answer"));
        assert_eq!(a[a.len() - 2], "--");
        assert_eq!(a[a.len() - 1], "--dangerous-looking answer");
        assert_eq!(a.iter().filter(|x| *x == "--").count(), 1);
    }

    #[test]
    fn grill_tools_are_scoped_and_have_no_shell() {
        let (p, d, f) = dirs();
        let a = turn_args(&spec(false, Step::Grill, &p, &d, &f, "x"));
        let allow = list(&a, "--allowedTools");
        assert_eq!(
            allow,
            vec![
                "Read(./**)", "Glob(./**)", "Grep(./**)", "Read(//private/var/T/shape-board/u/proto/**)",
                "Read(~/.claude/plugins/**)", "Write(./specs/**)", "Edit(./specs/**)"
            ]
        );
        // No bare tool name that would allow every path, and no Bash rule.
        for t in &allow {
            assert!(t.contains('('), "bare tool {t}");
            assert!(!t.starts_with("Bash"), "{t}");
        }
        let deny = list(&a, "--disallowedTools");
        for t in ["AskUserQuestion", "Bash", "Agent", "WebFetch", "WebSearch", "NotebookEdit", "Read(./.env*)", "Read(./**/.env*)", "Grep(./**/.env*)"] {
            assert!(deny.contains(&t), "{t} not denied");
        }
        assert_eq!(after(&a, "--add-dir"), "/private/var/T/shape-board/u/proto");
    }

    #[test]
    fn the_adr_turn_adds_only_the_decisions_folder_and_still_no_shell() {
        let (p, d, f) = dirs();
        let a = turn_args(&spec(false, Step::Adr, &p, &d, &f, "x"));
        let allow = list(&a, "--allowedTools");
        let grill = allowed_tools(Step::Grill, &p, &d);
        let extra: Vec<&str> = allow.iter().copied().filter(|t| !grill.iter().any(|g| g == t)).collect();
        assert_eq!(
            extra,
            vec![
                "Read(//private/var/T/shape-board/u/decisions/**)",
                "Write(//private/var/T/shape-board/u/decisions/**)",
                "Edit(//private/var/T/shape-board/u/decisions/**)"
            ]
        );
        assert!(list(&a, "--disallowedTools").contains(&"Bash"));
        assert!(!allow.iter().any(|t| t.contains("docs/adr") || t.starts_with("Bash")));
    }

    #[test]
    fn the_app_prompt_names_the_headers_and_the_decisions_file() {
        let p = app_prompt(Path::new("/t/decisions/decisions.json"));
        assert!(p.starts_with(SPIKE_PROMPT.trim_end()));
        assert!(p.contains("\"Brief\"") && p.contains("\"ADR\""));
        assert!(p.contains("/t/decisions/decisions.json"));
        assert!(p.contains("do not run git or gh"));
    }

    #[test]
    fn display_command_is_one_short_line() {
        let (p, d, f) = dirs();
        assert_eq!(
            display_command(&spec(true, Step::Grill, &p, &d, &f, "/pd-bot:pd-shape /x/prototype.html")),
            "claude -p /pd-bot:pd-shape --session-id 11111111"
        );
        assert_eq!(display_command(&spec(false, Step::Adr, &p, &d, &f, "{}")), "claude -p (answers) --resume 11111111 (ADR turn)");
    }
}
