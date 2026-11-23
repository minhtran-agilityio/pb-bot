//! The engine against a stand-in `claude` (a Python script named `claude` that prints stream-json).
//! No real Claude, no quota. The real run is the e2e driver (`src/bin/grill-e2e.rs`).

use super::engine::*;
use super::store;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FAKE: &str = r##"#!/usr/bin/env python3
import json, os, subprocess, sys
d = os.environ["FAKE_DIR"]
args = sys.argv[1:]
open(os.path.join(d, "args.log"), "a").write(json.dumps(args) + "\n")
mf = os.path.join(d, "mode")
mode = open(mf).read().strip() if os.path.exists(mf) else "q"
def out(o): print(json.dumps(o), flush=True)
if mode == "noconv" and "--resume" in args:
    print("No conversation found with session ID: x", file=sys.stderr); sys.exit(1)
out({"type": "system", "subtype": "init", "session_id": "x", "permissionMode": "auto" if mode == "badmode" else "dontAsk"})
if mode == "sleep":
    p = subprocess.Popen(["sleep", "30"])
    open(os.path.join(d, "child.pid"), "w").write(str(p.pid))
    p.wait(); sys.exit(0)
if mode == "limit":
    out({"type": "result", "subtype": "success", "is_error": True, "result": "Claude AI usage limit reached|1764000000"}); sys.exit(1)
def block(h, q):
    return "```p2g-questions\n" + json.dumps({"questions": [{"question": q, "header": h, "multiSelect": False,
        "options": [{"label": "Yes (Recommended)", "description": "a"}, {"label": "No", "description": "b"}]}]}) + "\n```"
if mode == "brief":
    os.makedirs("specs/phone", exist_ok=True)
    open("specs/phone/brief.md", "w").write("# Brief: Phone sign-up\n\n## Goal\nSign up with a phone.\n")
    text = "I wrote the brief.\n\n" + block("Brief", "Does this brief say what you mean?")
elif mode == "done":
    text = "Thanks. The brief is final."
elif mode == "adr":
    text = "One repo-level decision.\n\n" + block("ADR", "Make phone-only an ADR?")
elif mode == "badblock":
    text = "Oops.\n\n```p2g-questions\n{not json}\n```"
else:
    text = "First question.\n\n" + block("Scope", "Which sign-up methods?")
out({"type": "assistant", "parent_tool_use_id": None, "message": {"content": [{"type": "text", "text": text},
    {"type": "tool_use", "name": "Read", "input": {"file_path": "prototype.html"}}]}})
out({"type": "result", "subtype": "success", "is_error": False, "result": text, "total_cost_usd": 0.01})
"##;

struct T {
    _dir: tempfile::TempDir,
    fake_dir: PathBuf,
    engine: Engine,
    version: i64,
}

fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let (fake_dir, repo, tmp) = (dir.path().join("fake"), dir.path().join("repo"), dir.path().join("tmp"));
    for p in [&fake_dir, &repo, &tmp] {
        std::fs::create_dir_all(p).unwrap();
    }
    let claude = fake_dir.join("claude");
    std::fs::write(&claude, FAKE).unwrap();
    std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
    crate::db::insert_project(&conn, "a/b", "b", &repo.to_string_lossy()).unwrap();
    conn.execute("INSERT INTO artifacts (project_id, name, source, state) VALUES (1, 'proto', 'manual', 'added')", []).unwrap();
    conn.execute("INSERT INTO artifact_versions (artifact_id, n, html, sha256, source) VALUES (1, 1, '<p>proto</p>', 'h', 'paste')", []).unwrap();
    let mut engine = Engine::new(Arc::new(Mutex::new(conn)), dir.path().to_path_buf(), Arc::new(|_| {}));
    engine.claude = claude.to_string_lossy().into_owned();
    // A stand-in pd-bot plugin: its SKILL.md frontmatter grants bare Write, which must never reach Claude.
    let plugin = dir.path().join("plugin");
    std::fs::create_dir_all(plugin.join("skills/pd-shape")).unwrap();
    std::fs::write(plugin.join("skills/pd-shape/SKILL.md"), "---\nname: pd-shape\nallowed-tools: Read, Write, Edit\n---\n# Shape\nRead ${CLAUDE_PLUGIN_ROOT}/references/questions.md\n").unwrap();
    engine.plugin_root = Some(plugin);
    engine.tmp_root = tmp.canonicalize().unwrap();
    engine.env = vec![("FAKE_DIR".into(), fake_dir.to_string_lossy().into_owned())];
    T { _dir: dir, fake_dir, engine, version: 1 }
}

impl T {
    fn mode(&self, m: &str) {
        std::fs::write(self.fake_dir.join("mode"), m).unwrap();
    }
    fn state(&self, id: i64) -> String {
        store::summary(&self.engine.db.lock().unwrap(), id).unwrap().unwrap().state
    }
    fn wait(&self, id: i64) -> String {
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            let s = self.state(id);
            if s != "running" {
                // The thread clears the live slot right after the last write.
                std::thread::sleep(Duration::from_millis(50));
                return s;
            }
            assert!(Instant::now() < end, "still running");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn detail(&self, id: i64) -> store::SessionDetail {
        store::detail(&self.engine.db.lock().unwrap(), id).unwrap()
    }
    fn args(&self) -> Vec<Vec<String>> {
        std::fs::read_to_string(self.fake_dir.join("args.log"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.engine.db.lock().unwrap()
    }
}

fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

#[test]
fn a_grill_runs_from_question_to_brief_to_confirmed() {
    let t = setup();
    t.mode("q");
    let id = t.engine.start(t.version).unwrap();
    assert_eq!(t.wait(id), "waiting_owner");
    let d = t.detail(id);
    let ob = d.open_block.clone().expect("a question card");
    assert_eq!(ob.questions["questions"][0]["question"], "Which sign-up methods?");
    // The block is cut out of the chat text; the tool call is its own row.
    let claude: Vec<_> = d.messages.iter().filter(|m| m.role == "claude").collect();
    assert_eq!(claude[0].text, "First question.");
    assert!(d.messages.iter().any(|m| m.role == "tool" && m.text == "Read prototype.html"));
    let a = &t.args()[0];
    assert!(has(a, "--session-id") && !has(a, "--resume"));
    // The skill text, never the slash command (its allowed-tools would widen the fence).
    let first = a.last().unwrap();
    assert!(first.starts_with(super::flags::FIRST_PREFIX), "{first}");
    assert!(!first.contains("allowed-tools"));
    assert!(first.contains("/plugin/references/questions.md"));
    assert!(first.ends_with("/proto/prototype.html"));
    // The prototype was written for Claude.
    let proto = PathBuf::from(first.rsplit("ARGUMENTS: ").next().unwrap());
    assert_eq!(std::fs::read_to_string(&proto).unwrap(), "<p>proto</p>");
    assert_eq!(d.runs.len(), 1);
    assert!(d.runs[0].command.starts_with("claude -p pd-shape --session-id"));

    // The answer goes back as {"answers": ...} with --resume; the turn writes the brief.
    t.mode("brief");
    assert!(t.engine.answer(id, "Which sign-up methods?", "Phone only", false).unwrap());
    assert_eq!(t.wait(id), "brief_ready");
    let a = &t.args()[1];
    assert!(has(a, "--resume") && !has(a, "--session-id"));
    assert_eq!(a.last().unwrap(), r#"{"answers":{"Which sign-up methods?":"Phone only"}}"#);
    let d = t.detail(id);
    let b = d.brief.clone().unwrap();
    assert_eq!((b.n, b.path.as_str(), b.confirmed), (1, "specs/phone/brief.md", false));
    assert_eq!(d.session.feature_name.as_deref(), Some("Phone sign-up"));

    // Confirm brief: the feature exists at once; the yes picks the yes-like label.
    t.mode("done");
    t.engine.confirm_brief(id).unwrap();
    assert!(store::feature(&t.conn(), id).unwrap().is_some());
    assert_eq!(t.wait(id), "confirmed");
    assert_eq!(t.args()[2].last().unwrap(), r#"{"answers":{"Does this brief say what you mean?":"Yes (Recommended)"}}"#);
    let d = t.detail(id);
    assert!(d.brief.unwrap().confirmed);
    assert_eq!(d.feature.unwrap().name, "Phone sign-up");
    assert_eq!(d.session.answers, 2, "Phone only + Confirm brief");
}

#[test]
fn a_usage_limit_fails_with_the_real_text_and_resume_wraps_the_answer() {
    let t = setup();
    t.mode("q");
    let id = t.engine.start(t.version).unwrap();
    t.wait(id);
    t.mode("limit");
    t.engine.answer(id, "Which sign-up methods?", "Phone only", false).unwrap();
    assert_eq!(t.wait(id), "failed");
    let d = t.detail(id);
    assert_eq!(d.error.as_deref(), Some("Claude AI usage limit reached|1764000000"));
    assert!(d.messages.iter().any(|m| m.role == "error" && m.text == "Claude AI usage limit reached|1764000000"));
    assert!(d.has_pending);
    assert_eq!(d.runs.last().unwrap().exit_code, Some(1));
    // The owner cannot answer a failed session; Resume sends the wrapped answer.
    assert!(t.engine.send_text(id, "hi").is_err());
    t.mode("brief");
    t.engine.resume(id).unwrap();
    assert_eq!(t.wait(id), "brief_ready");
    let last = t.args().last().unwrap().last().unwrap().clone();
    assert!(last.starts_with("The last turn was interrupted"), "{last}");
    assert!(last.contains(r#"{"answers":{"Which sign-up methods?":"Phone only"}}"#));
    assert!(!t.detail(id).has_pending);
}

#[test]
fn stop_kills_the_whole_group_and_resume_continues() {
    let t = setup();
    t.mode("sleep");
    let id = t.engine.start(t.version).unwrap();
    let pidf = t.fake_dir.join("child.pid");
    let end = Instant::now() + Duration::from_secs(10);
    while !pidf.exists() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
    let child: i32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
    // Only one turn in the app at a time.
    assert!(t.engine.resume(id).is_err());
    t.engine.stop(id).unwrap();
    assert_eq!(t.wait(id), "stopped");
    assert!(!Path::new(&format!("/proc/{child}")).exists() || !alive(child), "the child sleep survived Stop");
    let d = t.detail(id);
    assert_eq!(d.messages.last().unwrap().text, NOTE_STOPPED);
    assert!(d.has_pending);
    // Claude started (init came), so Resume uses --resume and sends the first command again.
    t.mode("q");
    t.engine.resume(id).unwrap();
    assert_eq!(t.wait(id), "waiting_owner");
    let a = t.args().last().unwrap().clone();
    assert!(has(&a, "--resume"));
    assert!(a.last().unwrap().starts_with(super::flags::FIRST_PREFIX));
}

fn alive(pid: i32) -> bool {
    std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().is_ok_and(|s| s.success())
}

#[test]
fn a_turn_that_runs_too_long_fails_and_its_group_is_killed() {
    let mut t = setup();
    t.engine.turn_timeout = Duration::from_millis(500);
    t.mode("sleep");
    let id = t.engine.start(t.version).unwrap();
    assert_eq!(t.wait(id), "failed");
    let child: i32 = std::fs::read_to_string(t.fake_dir.join("child.pid")).unwrap().trim().parse().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(!alive(child));
    assert!(t.detail(id).error.unwrap().contains("did not finish this turn in 15 minutes"));
}

#[test]
fn a_session_in_another_permission_mode_is_stopped() {
    let t = setup();
    t.mode("badmode");
    let id = t.engine.start(t.version).unwrap();
    assert_eq!(t.wait(id), "failed");
    assert!(t.detail(id).error.unwrap().contains("permission mode \"auto\""));
}

#[test]
fn no_conversation_found_retries_once_with_session_id() {
    let t = setup();
    t.mode("q");
    let id = t.engine.start(t.version).unwrap();
    t.wait(id);
    t.mode("noconv");
    t.engine.answer(id, "Which sign-up methods?", "Phone only", false).unwrap();
    assert_eq!(t.wait(id), "waiting_owner");
    let a = t.args();
    assert!(has(&a[1], "--resume"));
    assert!(has(&a[2], "--session-id"));
}

#[test]
fn an_unreadable_block_lets_the_owner_answer_in_text() {
    let t = setup();
    t.mode("badblock");
    let id = t.engine.start(t.version).unwrap();
    assert_eq!(t.wait(id), "waiting_owner");
    let d = t.detail(id);
    assert!(d.open_block.is_none());
    assert!(d.messages.iter().any(|m| m.role == "error" && m.text.contains("could not read: invalid JSON")));
    t.mode("q");
    t.engine.send_text(id, "Phone only, please.").unwrap();
    assert_eq!(t.wait(id), "waiting_owner");
    assert_eq!(t.args().last().unwrap().last().unwrap(), "Phone only, please.");
}

#[test]
fn an_adr_yes_needs_the_dialog_and_a_no_never_starts_the_adr_step() {
    let t = setup();
    t.mode("adr");
    let id = t.engine.start(t.version).unwrap();
    t.wait(id);
    let q = "Make phone-only an ADR?";
    assert_eq!(t.engine.answer(id, q, "Yes (Recommended)", false).unwrap_err(), "Confirm the ADR in the dialog first.");
    assert_eq!(t.engine.answer(id, q, "No", true).unwrap_err(), "Only a yes to an ADR question can start the ADR step.");
    // Nothing was recorded by the refused answers.
    assert!(t.detail(id).open_block.unwrap().drafts.as_object().unwrap().is_empty());
    // A no is an ordinary grill turn: no decisions folder rules, no ADR run.
    t.mode("q");
    t.engine.answer(id, q, "No", false).unwrap();
    t.wait(id);
    let a = t.args().last().unwrap().clone();
    assert!(!a.iter().any(|x| x.contains("/decisions/**")));
    assert!(!t.detail(id).adr_pending);
}

#[test]
fn an_adr_yes_runs_an_adr_turn_and_the_app_reports_the_result() {
    let t = setup();
    t.mode("adr");
    let id = t.engine.start(t.version).unwrap();
    t.wait(id);
    t.mode("q");
    t.engine.answer(id, "Make phone-only an ADR?", "Yes (Recommended)", true).unwrap();
    t.wait(id);
    let a = t.args().last().unwrap().clone();
    assert!(a.iter().any(|x| x.starts_with("Edit(//") && x.ends_with("/decisions/**)")), "{a:?}");
    assert!(a.iter().any(|x| x == "Bash"), "Bash stays denied");
    // The stand-in wrote no decisions file: the app makes no ADR and says so, and tells Claude next turn.
    let d = t.detail(id);
    assert!(!d.adr_pending);
    assert!(d.adr_review.is_none());
    assert!(t.engine.adr_decide(id, true).unwrap_err().contains("no ADR waiting"));
    let note = d.messages.iter().rev().find(|m| m.role == "note").unwrap().text.clone();
    assert!(note.starts_with("ADR step failed"), "{note}");
    t.engine.answer(id, "Which sign-up methods?", "Phone only", false).unwrap();
    t.wait(id);
    assert!(t.args().last().unwrap().last().unwrap().starts_with("App note: ADR step failed"));
}

#[test]
fn partial_answers_survive_and_one_block_sends_once() {
    let t = setup();
    t.mode("q");
    let id = t.engine.start(t.version).unwrap();
    t.wait(id);
    // Replace the open block with a two-question one.
    let mid = t.detail(id).open_block.unwrap().message_id;
    let two = serde_json::json!({"questions": [
        {"question": "A?", "header": "a", "multiSelect": false, "options": [{"label": "1", "description": ""}, {"label": "2", "description": ""}]},
        {"question": "B?", "header": "b", "multiSelect": true, "options": [{"label": "x", "description": ""}, {"label": "y", "description": ""}]}]});
    t.conn().execute("UPDATE grill_messages SET questions = ?2 WHERE id = ?1", rusqlite::params![mid, two.to_string()]).unwrap();
    assert!(!t.engine.answer(id, "A?", "1", false).unwrap());
    assert_eq!(t.detail(id).open_block.unwrap().drafts["A?"], "1");
    assert_eq!(t.args().len(), 1, "nothing sent yet");
    assert!(t.engine.answer(id, "B?", "x, y", false).unwrap());
    t.wait(id);
    assert_eq!(t.args()[1].last().unwrap(), r#"{"answers":{"A?":"1","B?":"x, y"}}"#);
}

/// The crash case: a claude group is still running and no engine thread owns it (the app was killed).
/// The next app start kills it and shows the session as stopped; Resume continues.
#[test]
fn the_next_app_start_kills_a_group_a_crash_left_and_resume_works() {
    let t = setup();
    t.mode("q");
    let id = t.engine.start(t.version).unwrap();
    t.wait(id);
    // Simulate the crashed app's turn: a real `claude` stand-in in its own group, recorded in runs.
    t.mode("sleep");
    let mut cmd = std::process::Command::new(&t.engine.claude);
    cmd.arg("x").env("FAKE_DIR", &t.fake_dir).stdout(std::process::Stdio::null());
    super::proc::own_group(&mut cmd);
    let mut orphan = cmd.spawn().unwrap();
    let pgid = orphan.id() as i32;
    let pidf = t.fake_dir.join("child.pid");
    let end = Instant::now() + Duration::from_secs(10);
    while !pidf.exists() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
    let child: i32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
    {
        let conn = t.conn();
        conn.execute("UPDATE grill_sessions SET state = 'running', turns = turns + 1, pending_message = 'x' WHERE id = ?1", [id]).unwrap();
        let run = store::start_run(&conn, id, "grill", "claude -p", &[]).unwrap();
        store::set_run_process(&conn, run, pgid, super::proc::start_time(pgid).as_deref()).unwrap();
    }
    // "Relaunch": a new engine on the same database.
    let mut again = Engine::new(t.engine.db.clone(), t.engine.home.clone(), Arc::new(|_| {}));
    again.claude = t.engine.claude.clone();
    again.tmp_root = t.engine.tmp_root.clone();
    again.env = t.engine.env.clone();
    assert_eq!(again.recover().unwrap(), 1);
    let _ = orphan.wait();
    std::thread::sleep(Duration::from_millis(100));
    assert!(!alive(child), "the orphan's child survived the sweep");
    assert_eq!(t.state(id), "stopped");
    assert_eq!(t.detail(id).messages.last().unwrap().text, store::NOTE_APP_CLOSED);
    t.mode("q");
    again.resume(id).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while t.state(id) == "running" {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(t.state(id), "waiting_owner");
    assert!(t.args().last().unwrap().last().unwrap().starts_with("The last turn was interrupted"));
}
