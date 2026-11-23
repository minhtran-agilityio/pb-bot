//! The `claude -p` command line of one grill turn: the flag set of `spike/README.md`, ported, with the
//! read rules narrowed and the ADR git work moved to the app (specs design §3, advisor must-fix 1 and 5).
//!
//! Probes in this build (haiku, `--permission-mode dontAsk`; outputs in the PR):
//! - a bare `Read` reads any file, so the read tools are scoped to the repo, the prototype folder and
//!   the plugin folder; `Read(//<abs>/**)` is the absolute form;
//! - a `*` inside a Bash rule also matches extra arguments, so no Bash rule is given to Claude at all;
//! - a skill's `allowed-tools` (pd-shape lists bare `Read, Write, Edit`) DOES widen the fence when the
//!   turn starts with its slash command, even under dontAsk; so the app never types `/pd-bot:pd-shape`.
//!   It sends the skill's text as the first message (`first_message`) and denies the `Skill` tool;
//! - `Write(./specs/**)` matches nothing; `Edit(./specs/**)` allows Write and Edit there and nowhere else;
//! - allow rules in a trusted repo's `.claude/settings.json` or the user's settings would widen the fence,
//!   so the turn loads no settings file (`--setting-sources ""`).

use std::path::Path;

/// The spike's protocol text, verbatim (one source of truth).
pub const SPIKE_PROMPT: &str = include_str!("../../../spike/append-system-prompt.md");

/// Model and effort of the spike runs.
pub const MODEL: &str = "sonnet";
pub const EFFORT: &str = "medium";

/// Exact headers the app recognizes (rule 8 below).
pub const HEADER_BRIEF: &str = "Brief";
pub const HEADER_ADR: &str = "ADR";

/// The start of the first message; Resume sends a first message again unchanged.
pub const FIRST_PREFIX: &str = "Run the pd-bot skill pd-shape below";

/// The first message: pd-shape's own SKILL.md text (frontmatter dropped, so its `allowed-tools` grant
/// nothing), with `${CLAUDE_PLUGIN_ROOT}` filled in, and the prototype path as its argument. The skill is
/// reused, not rewritten (HANDOFF §3).
pub fn first_message(skill_md: &str, plugin_root: &Path, prototype: &Path) -> String {
    let body = strip_frontmatter(skill_md).replace("${CLAUDE_PLUGIN_ROOT}", &plugin_root.display().to_string());
    let body = body.replace("$ARGUMENTS", &prototype.display().to_string());
    format!(
        "{FIRST_PREFIX} (Mode: interactive) on the prototype {proto}. The skill's own files are in {skill}: a path such as \
         `references/questions.md` in the text means {skill}/references/questions.md. Read them with the Read tool. \
         The skill text is between the two lines.\n\n----\n{body}\n----\n\nARGUMENTS: {proto}",
        proto = prototype.display(),
        skill = plugin_root.join("skills/pd-shape").display(),
        body = body.trim()
    )
}

/// The text after a leading `---` frontmatter block.
pub fn strip_frontmatter(md: &str) -> &str {
    let Some(rest) = md.strip_prefix("---\n") else { return md };
    match rest.find("\n---\n") {
        Some(i) => &rest[i + 5..],
        None => md,
    }
}
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
    // An Edit rule covers Write too (probe 7); a Write rule matches nothing.
    v.push("Edit(./specs/**)".into());
    if step == Step::Adr {
        v.push(abs_rule("Read", decisions_dir));
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
        // A model-invoked skill would bring its own allowed-tools grant (probe 5).
        "Skill",
    ]
    .iter()
    .map(|s| s.to_string())
    // Secrets in the repo (deny beats allow; Grep and Glob path rules are best effort).
    .chain(SECRET_GLOBS.iter().flat_map(|g| ["Read", "Grep", "Glob"].map(|t| format!("{t}(./{g})"))))
    .collect()
}

/// Files a grill never needs: env files, the git folder (a remote URL can hold a token), keys and
/// credential files.
pub const SECRET_GLOBS: &[&str] = &[
    ".env*", "**/.env*", ".git/**", "**/*.pem", "**/*.key", "**/id_rsa*", "**/id_ed25519*", "**/id_ecdsa*",
    "**/credentials*", "**/.npmrc", "**/.netrc", "**/.pypirc",
];

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
        // No settings file: no allow rule, hook or plugin from the user's or the repo's settings.
        "--setting-sources".into(),
        "".into(),
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
    let what = if t.message.starts_with(FIRST_PREFIX) {
        "pd-shape".to_string()
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
                "Read(~/.claude/plugins/**)", "Edit(./specs/**)"
            ]
        );
        // No bare tool name that would allow every path, and no Bash rule.
        for t in &allow {
            assert!(t.contains('('), "bare tool {t}");
            assert!(!t.starts_with("Bash"), "{t}");
        }
        let deny = list(&a, "--disallowedTools");
        for t in ["AskUserQuestion", "Bash", "Agent", "WebFetch", "WebSearch", "NotebookEdit", "Skill", "Read(./.env*)", "Read(./**/.env*)",
                  "Grep(./**/.env*)", "Read(./.git/**)", "Grep(./.git/**)", "Read(./**/id_rsa*)", "Read(./**/*.pem)"] {
            assert!(deny.contains(&t), "{t} not denied");
        }
        assert_eq!(after(&a, "--add-dir"), "/private/var/T/shape-board/u/proto");
        assert_eq!(after(&a, "--setting-sources"), "", "no settings file may add allow rules");
        // Nothing allows a write outside specs/.
        assert!(!allow.iter().any(|t| t.starts_with("Write")));
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
            vec!["Read(//private/var/T/shape-board/u/decisions/**)", "Edit(//private/var/T/shape-board/u/decisions/**)"]
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
    fn the_first_message_is_the_skill_text_without_its_frontmatter() {
        let md = "---\nname: pd-shape\nallowed-tools: Read, Write, Edit, Bash(git push *)\n---\n\n# Shape\nRead `${CLAUDE_PLUGIN_ROOT}/references/adr.md`.\nArgs: $ARGUMENTS\n";
        let m = first_message(md, Path::new("/home/u/.claude/plugins/cache/pd-bot/pd-bot/0.6.0"), Path::new("/t/proto/prototype.html"));
        assert!(m.starts_with(FIRST_PREFIX));
        assert!(!m.contains("allowed-tools"), "the frontmatter grant must not reach Claude as an instruction");
        assert!(m.contains("Read `/home/u/.claude/plugins/cache/pd-bot/pd-bot/0.6.0/references/adr.md`."));
        assert!(m.contains("Args: /t/proto/prototype.html"));
        assert!(m.contains("means /home/u/.claude/plugins/cache/pd-bot/pd-bot/0.6.0/skills/pd-shape/references/questions.md"));
        assert!(m.ends_with("ARGUMENTS: /t/proto/prototype.html"));
        assert!(!m.starts_with('/'), "never a slash command: the skill's grant would apply");
        assert_eq!(strip_frontmatter("no frontmatter"), "no frontmatter");
    }

    #[test]
    fn display_command_is_one_short_line() {
        let (p, d, f) = dirs();
        let first = first_message("---\nname: x\n---\nbody", Path::new("/r"), Path::new("/x/prototype.html"));
        assert_eq!(display_command(&spec(true, Step::Grill, &p, &d, &f, &first)), "claude -p pd-shape --session-id 11111111");
        assert_eq!(display_command(&spec(false, Step::Adr, &p, &d, &f, "{}")), "claude -p (answers) --resume 11111111 (ADR turn)");
    }
}
