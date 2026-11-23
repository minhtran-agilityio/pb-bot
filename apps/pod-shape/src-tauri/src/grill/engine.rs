//! The grill engine: one `claude -p` process per turn (the `claude_runner` of DESIGN_DOC), the session
//! state machine of specs design §2.4, Stop, Resume, the app-start recovery and the owner's answers.
//! The database is the source of truth; the UI reloads a session on each `notify(session id)`.

use super::brief;
use super::flags::{self, Step, TurnSpec};
use super::p2g;
use super::proc;
use super::store::{self, TurnContext};
use super::stream::{self, Event, TurnResult};
use rusqlite::Connection;
use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// The spike's per-turn limit.
pub const TURN_TIMEOUT: Duration = Duration::from_secs(900);
/// Stop: SIGTERM, then SIGKILL after this.
pub const STOP_GRACE: Duration = Duration::from_secs(3);

pub const NOTE_STOPPED: &str = "Stopped. Press Resume to continue the same Claude session.";
pub const CONFIRM_TEXT: &str = "Yes, the brief says what I mean.";

/// The running turn (at most one in the whole app).
struct Live {
    session: i64,
    pgid: i32,
    stop: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct Engine {
    pub db: Arc<Mutex<Connection>>,
    /// `<canonical temp dir>/shape-board`.
    pub tmp_root: PathBuf,
    /// The `claude` program (a stand-in in tests).
    pub claude: String,
    pub home: PathBuf,
    /// The pd-bot plugin whose pd-shape SKILL.md starts a grill (pinned marketplace, adr::plugin_root).
    pub plugin_root: Option<PathBuf>,
    pub turn_timeout: Duration,
    /// Extra environment for the child (tests).
    pub env: Vec<(String, String)>,
    live: Arc<Mutex<Option<Live>>>,
    notify: Arc<dyn Fn(i64) + Send + Sync>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn e(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A random v4 uuid from /dev/urandom.
pub fn new_uuid() -> Result<String, String> {
    use std::io::Read;
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).map_err(e)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]))
}

/// The temp root for prototype copies and ADR scratch: canonical (macOS /var is a link to /private/var).
pub fn default_tmp_root() -> PathBuf {
    let t = std::env::temp_dir();
    t.canonicalize().unwrap_or(t).join("shape-board")
}

/// Folders of one session under the temp root.
pub struct Dirs {
    pub scratch: PathBuf,
    pub proto: PathBuf,
    pub decisions: PathBuf,
    pub decisions_file: PathBuf,
}

pub fn dirs(root: &Path, uuid: &str) -> Dirs {
    let scratch = root.join(uuid);
    Dirs {
        proto: scratch.join("proto"),
        decisions: scratch.join("decisions"),
        decisions_file: scratch.join("decisions").join("decisions.json"),
        scratch,
    }
}

/// Is this a question about the brief (header "Brief", or the text names the brief)?
fn is_brief_question(q: &Value) -> bool {
    let header = q.get("header").and_then(Value::as_str).unwrap_or("").trim();
    header.eq_ignore_ascii_case(flags::HEADER_BRIEF)
        || q.get("question").and_then(Value::as_str).is_some_and(|t| t.to_lowercase().contains("brief"))
}

fn is_adr_question(q: &Value) -> bool {
    q.get("header").and_then(Value::as_str).unwrap_or("").trim().eq_ignore_ascii_case(flags::HEADER_ADR)
}

/// A yes to an ADR: the label starts with Yes or Make (fail closed: "Brief only", "Skip" are no).
pub fn is_adr_yes(answer: &str) -> bool {
    let a = answer.trim_start().to_lowercase();
    ["yes", "make"].iter().any(|w| a.starts_with(w) && a[w.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric()))
}

/// The session state after a successful turn (specs design §2.4 and §5).
pub fn next_state(block: Option<&Value>, brief_exists: bool, brief_confirmed: bool) -> &'static str {
    match block {
        Some(b) => {
            let asks_brief = b.get("questions").and_then(Value::as_array).is_some_and(|qs| qs.iter().any(is_brief_question));
            if asks_brief && brief_exists && !brief_confirmed {
                "brief_ready"
            } else {
                "waiting_owner"
            }
        }
        None if brief_exists && brief_confirmed => "confirmed",
        None if brief_exists => "brief_ready",
        None => "waiting_owner",
    }
}

/// The message that resumes an interrupted turn (advisor must-fix 4): never the old answers word for
/// word, because Claude may have recorded or acted on them.
pub fn resume_message(pending: Option<&str>) -> String {
    match pending {
        Some(p) if p.starts_with(flags::FIRST_PREFIX) => p.to_string(),
        Some(p) => format!(
            "The last turn was interrupted before it finished. My message was:\n{p}\nIf you already acted on it, continue from where you were."
        ),
        None => "Continue.".to_string(),
    }
}

impl Engine {
    pub fn new(db: Arc<Mutex<Connection>>, home: PathBuf, notify: Arc<dyn Fn(i64) + Send + Sync>) -> Engine {
        Engine {
            db,
            tmp_root: default_tmp_root(),
            // A test stand-in only in a debug build.
            claude: std::env::var("SHAPE_BOARD_CLAUDE").ok().filter(|_| cfg!(debug_assertions)).unwrap_or_else(|| "claude".into()),
            plugin_root: super::adr::plugin_root(&home),
            home,
            turn_timeout: TURN_TIMEOUT,
            env: vec![],
            live: Arc::new(Mutex::new(None)),
            notify,
        }
    }

    fn changed(&self, id: i64) {
        (self.notify)(id);
    }

    /// App start: kill groups a crash left behind, then running sessions become stopped (§2.5).
    pub fn recover(&self) -> Result<usize, String> {
        let live = store::live_runs(&lock(&self.db)).map_err(e)?;
        for (pgid, start) in live {
            if proc::kill_orphan(pgid, start.as_deref()) {
                eprintln!("shape-board: killed a claude group left by an earlier run ({pgid})");
            }
        }
        store::recover_running(&lock(&self.db)).map_err(e)
    }

    /// App exit (Cmd+Q, window close): kill the running turn's group. The session stays `running` in
    /// the database, so the next start shows it as stopped and offers Resume.
    pub fn shutdown(&self) {
        if let Some(l) = lock(&self.live).as_ref() {
            l.stop.store(true, Ordering::SeqCst);
            proc::terminate_group(l.pgid, Duration::from_millis(500));
        }
    }

    // ---------- owner actions ----------

    /// Starts a grill on one artifact version: a new session and its first turn.
    pub fn start(&self, version_id: i64) -> Result<i64, String> {
        let uuid = new_uuid()?;
        let d = dirs(&self.tmp_root, &uuid);
        // Never the slash command: its allowed-tools would widen the fence (security review finding 1).
        let root = self.plugin_root.clone().ok_or("pd-bot's pd-shape skill was not found under ~/.claude/plugins. Install or update the pd-bot plugin.")?;
        let skill = std::fs::read_to_string(root.join("skills/pd-shape/SKILL.md")).map_err(|x| format!("pd-shape's SKILL.md could not be read: {x}"))?;
        let message = flags::first_message(&skill, &root, &d.proto.join("prototype.html"));
        let l = {
            let conn = lock(&self.db);
            self.check_free(&conn)?;
            let id = store::create_session(&conn, version_id, &uuid)?;
            store::add_message(&conn, id, 0, "note", "Grill started. Claude reads the prototype first; this takes about half a minute.", None).map_err(e)?;
            self.claim(&conn, id, message, false)?
        };
        let id = l.id;
        self.launch(l);
        Ok(id)
    }

    /// One answer to one question of the open block. The turn is sent when every question has an
    /// answer. `adr` is true only after the owner confirmed the ADR dialog. Returns whether it sent.
    pub fn answer(&self, id: i64, question: &str, answer: &str, adr: bool) -> Result<bool, String> {
        let answer = answer.trim();
        if answer.is_empty() {
            return Err("Write an answer first.".into());
        }
        let l = {
            let conn = lock(&self.db);
            self.check_free(&conn)?;
            let tc = store::turn_context(&conn, id)?;
            if !matches!(tc.state.as_str(), "waiting_owner" | "brief_ready") {
                return Err("This session is not waiting for an answer.".into());
            }
            let ob = store::open_block(&conn, id).map_err(e)?.ok_or("There is no open question.")?;
            let qs = ob.questions.get("questions").and_then(Value::as_array).cloned().unwrap_or_default();
            let q = qs
                .iter()
                .find(|q| q.get("question").and_then(Value::as_str) == Some(question))
                .ok_or("That question is not open.")?;
            let adr_q = is_adr_question(q) && is_adr_yes(answer);
            if adr_q && !adr {
                return Err("Confirm the ADR in the dialog first.".into());
            }
            if adr && !adr_q {
                return Err("Only a yes to an ADR question can start the ADR step.".into());
            }
            let mut drafts: Map<String, Value> = ob.drafts.as_object().cloned().unwrap_or_default();
            drafts.insert(question.to_string(), json!(answer));
            store::add_message(&conn, id, tc.turns, "owner", answer, None).map_err(e)?;
            if adr {
                conn.execute("UPDATE grill_sessions SET adr_pending = 1 WHERE id = ?1", [id]).map_err(e)?;
            }
            let all = qs.iter().all(|q| q.get("question").and_then(Value::as_str).is_some_and(|t| drafts.contains_key(t)));
            if !all {
                store::set_draft(&conn, id, Some(&Value::Object(drafts))).map_err(e)?;
                drop(conn);
                self.changed(id);
                return Ok(false);
            }
            store::mark_answered(&conn, ob.message_id).map_err(e)?;
            store::set_draft(&conn, id, None).map_err(e)?;
            self.claim(&conn, id, json!({ "answers": drafts }).to_string(), adr || tc.adr_pending)?
        };
        self.launch(l);
        Ok(true)
    }

    /// Free text when no question block is open (Claude asked in prose, or the brief waits).
    pub fn send_text(&self, id: i64, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Write a message first.".into());
        }
        let l = {
            let conn = lock(&self.db);
            self.check_free(&conn)?;
            let tc = store::turn_context(&conn, id)?;
            if !matches!(tc.state.as_str(), "waiting_owner" | "brief_ready") {
                return Err("This session is not waiting for you.".into());
            }
            if store::open_block(&conn, id).map_err(e)?.is_some() {
                return Err("Answer the open question first.".into());
            }
            store::add_message(&conn, id, tc.turns, "owner", text, None).map_err(e)?;
            self.claim(&conn, id, text.to_string(), tc.adr_pending)?
        };
        self.launch(l);
        Ok(())
    }

    /// "Confirm brief": the owner's yes. The brief is confirmed and the feature made at once; then the
    /// yes goes to Claude so its session ends cleanly.
    pub fn confirm_brief(&self, id: i64) -> Result<(), String> {
        let l = {
            let conn = lock(&self.db);
            self.check_free(&conn)?;
            let tc = store::turn_context(&conn, id)?;
            if tc.state != "brief_ready" {
                return Err("There is no brief waiting for confirmation.".into());
            }
            let ob = store::open_block(&conn, id).map_err(e)?;
            let message = match &ob {
                None => CONFIRM_TEXT.to_string(),
                Some(ob) => {
                    let qs = ob.questions.get("questions").and_then(Value::as_array).cloned().unwrap_or_default();
                    let mut drafts: Map<String, Value> = ob.drafts.as_object().cloned().unwrap_or_default();
                    for q in &qs {
                        let text = q.get("question").and_then(Value::as_str).unwrap_or_default();
                        if is_brief_question(q) {
                            let yes = q
                                .get("options")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .filter_map(|o| o.get("label").and_then(Value::as_str))
                                .find(|l| {
                                    let l = l.to_lowercase();
                                    ["yes", "confirm", "looks good", "correct"].iter().any(|w| l.starts_with(w))
                                })
                                .unwrap_or(CONFIRM_TEXT);
                            drafts.insert(text.to_string(), json!(yes));
                        } else if !drafts.contains_key(text) {
                            return Err("Answer the other questions first.".into());
                        }
                    }
                    store::mark_answered(&conn, ob.message_id).map_err(e)?;
                    store::set_draft(&conn, id, None).map_err(e)?;
                    json!({ "answers": drafts }).to_string()
                }
            };
            store::confirm_brief(&conn, id)?;
            store::add_message(&conn, id, tc.turns, "owner", "Confirm brief", None).map_err(e)?;
            self.claim(&conn, id, message, tc.adr_pending)?
        };
        self.launch(l);
        Ok(())
    }

    pub fn stop(&self, id: i64) -> Result<(), String> {
        let pgid = {
            let g = lock(&self.live);
            match g.as_ref() {
                Some(l) if l.session == id => {
                    l.stop.store(true, Ordering::SeqCst);
                    l.pgid
                }
                _ => return Err("This session is not running.".into()),
            }
        };
        std::thread::spawn(move || proc::terminate_group(pgid, STOP_GRACE));
        Ok(())
    }

    pub fn resume(&self, id: i64) -> Result<(), String> {
        let l = {
            let conn = lock(&self.db);
            self.check_free(&conn)?;
            let tc = store::turn_context(&conn, id)?;
            if !matches!(tc.state.as_str(), "stopped" | "failed") {
                return Err("Only a stopped or failed session can be resumed.".into());
            }
            let pending: Option<String> =
                conn.query_row("SELECT pending_message FROM grill_sessions WHERE id = ?1", [id], |r| r.get(0)).map_err(e)?;
            self.claim(&conn, id, resume_message(pending.as_deref()), tc.adr_pending)?
        };
        self.launch(l);
        Ok(())
    }

    // ---------- the turn ----------

    /// Err when any turn runs in the app (checked under the DB lock the caller holds).
    fn check_free(&self, conn: &Connection) -> Result<(), String> {
        let running: i64 =
            conn.query_row("SELECT COUNT(*) FROM grill_sessions WHERE state = 'running'", [], |r| r.get(0)).map_err(e)?;
        if running > 0 || lock(&self.live).is_some() {
            return Err("Another grill is running. Stop it or wait for it to finish.".into());
        }
        Ok(())
    }

    /// Marks the session running. The caller holds the DB lock from its checks to here, so the
    /// app-wide "one turn" rule is checked and set atomically.
    fn claim(&self, conn: &Connection, id: i64, message: String, adr: bool) -> Result<Launch, String> {
        self.check_free(conn)?;
        let tc = store::turn_context(conn, id)?;
        // An ADR result from the last turn rides on this message.
        let message = match &tc.app_note {
            Some(n) => format!("App note: {n}\n\n{message}"),
            None => message,
        };
        let turn = tc.turns + 1;
        conn.execute(
            "UPDATE grill_sessions SET state = 'running', turns = ?2, pending_message = ?3, app_note = NULL, error = NULL,
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
            rusqlite::params![id, turn, message],
        )
        .map_err(e)?;
        Ok(Launch { id, turn, message, step: if adr { Step::Adr } else { Step::Grill }, tc })
    }

    /// Starts a claimed turn on its own thread.
    fn launch(&self, l: Launch) {
        self.changed(l.id);
        let me = self.clone();
        std::thread::spawn(move || me.run_turn(l.id, l.turn, &l.message, l.step, l.tc));
    }

    fn fail(&self, id: i64, turn: i64, text: &str) {
        let conn = lock(&self.db);
        let _ = store::mark_interrupted(&conn, id, turn);
        let _ = store::add_message(&conn, id, turn, "error", text, None);
        let _ = conn.execute("UPDATE grill_sessions SET error = ?2 WHERE id = ?1", rusqlite::params![id, text]);
        let _ = store::set_state(&conn, id, "failed");
    }

    /// Runs one turn to its end. Never panics the app: every error becomes a `failed` session.
    fn run_turn(&self, id: i64, turn: i64, message: &str, step: Step, tc: TurnContext) {
        let d = dirs(&self.tmp_root, &tc.uuid);
        if let Err(err) = write_prototype(&d, &tc.html) {
            self.fail(id, turn, &format!("The prototype could not be written for Claude: {err}"));
            *lock(&self.live) = None;
            self.changed(id);
            return;
        }
        let project = PathBuf::from(&tc.project_path);
        let before = brief::snapshot(&project);
        let mut first = !tc.claude_started;
        let mut attempt = 0;
        let outcome = loop {
            attempt += 1;
            let o = self.spawn_once(id, turn, message, step, first, &d, &project);
            // Session-flag fallback, both ways (advisor must-fix 4).
            let text = o.error_text();
            if attempt == 1 && !o.stopped && !o.timed_out {
                if !first && text.contains("No conversation found") {
                    first = true;
                    continue;
                }
                if first && text.contains("already in use") {
                    first = false;
                    continue;
                }
            }
            break o;
        };
        self.finish(id, turn, step, outcome, &project, &before, &d);
        *lock(&self.live) = None;
        self.changed(id);
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_once(&self, id: i64, turn: i64, message: &str, step: Step, first: bool, d: &Dirs, project: &Path) -> Attempt {
        let spec = TurnSpec {
            session_uuid: &dirs_uuid(d),
            first,
            step,
            proto_dir: &d.proto,
            decisions_dir: &d.decisions,
            decisions_file: &d.decisions_file,
            message,
        };
        let args = flags::turn_args(&spec);
        let display = flags::display_command(&spec);
        let t0 = Instant::now();
        let run = match store::start_run(&lock(&self.db), id, step.as_str(), &display, &args) {
            Ok(r) => r,
            Err(err) => return Attempt::error(format!("The run could not be saved: {err}")),
        };
        let mut cmd = crate::checks::command(&self.claude);
        cmd.args(&args)
            .current_dir(project)
            .env("PD_BOT_ROLE", "none")
            .env("STOP_EVIDENCE_GUARD", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        proc::own_group(&mut cmd);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(err) => {
                let msg = format!("claude could not start: {err}. Check that Claude Code is installed (claude --version).");
                let _ = store::end_run(&lock(&self.db), run, None, Some(&msg), None, t0.elapsed().as_millis());
                return Attempt::error(msg);
            }
        };
        let pgid = child.id() as i32;
        let stop = Arc::new(AtomicBool::new(false));
        *lock(&self.live) = Some(Live { session: id, pgid, stop: stop.clone() });
        let _ = store::set_run_process(&lock(&self.db), run, pgid, proc::start_time(pgid).as_deref());

        // Watchdog: the turn timeout.
        let timed_out = Arc::new(AtomicBool::new(false));
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let (to, limit) = (timed_out.clone(), self.turn_timeout);
        let watchdog = std::thread::spawn(move || {
            if let Err(mpsc::RecvTimeoutError::Timeout) = done_rx.recv_timeout(limit) {
                to.store(true, Ordering::SeqCst);
                proc::terminate_group(pgid, STOP_GRACE);
            }
        });
        let (stderr, err_h) = proc::tail_reader(child.stderr.take().expect("piped"), 8 * 1024);
        let mut result: Option<TurnResult> = None;
        let mut bad_mode: Option<String> = None;
        let mut last_text: Option<(i64, String)> = None;
        let stdout = child.stdout.take().expect("piped");
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            for ev in stream::parse_line(&line) {
                let conn = lock(&self.db);
                match ev {
                    Event::Init { permission_mode, .. } => {
                        if permission_mode != "dontAsk" {
                            bad_mode = Some(permission_mode);
                            proc::kill_group(pgid, proc::SIGKILL);
                        } else {
                            let _ = conn.execute("UPDATE grill_sessions SET claude_started = 1 WHERE id = ?1", [id]);
                        }
                    }
                    Event::Text(t) => {
                        if let Ok(mid) = store::add_message(&conn, id, turn, "claude", &t, None) {
                            last_text = Some((mid, t));
                        }
                    }
                    Event::ToolUse { name, summary, .. } => {
                        let _ = store::add_message(&conn, id, turn, "tool", format!("{name} {summary}").trim(), None);
                    }
                    Event::Result(r) => {
                        // A denied tool call is part of the history (and the proof that the fence held).
                        for d in &r.denials {
                            let _ = store::add_message(&conn, id, turn, "tool", &format!("Denied: {d}"), None);
                        }
                        result = Some(r);
                    }
                }
                drop(conn);
                self.changed(id);
            }
        }
        let status = child.wait().ok();
        // The group is gone: no later Stop or exit may signal its (reusable) id (security review finding 5).
        *lock(&self.live) = None;
        let _ = done_tx.send(());
        let _ = watchdog.join();
        let _ = err_h.join();
        let code = status.and_then(|s| s.code());
        let att = Attempt {
            code,
            result,
            stderr: proc::text_of(&stderr),
            stopped: stop.load(Ordering::SeqCst),
            timed_out: timed_out.load(Ordering::SeqCst),
            bad_mode,
            last_text,
            spawn_error: None,
        };
        let err = if att.success() { None } else { Some(att.error_text()) };
        let cost = att.result.as_ref().and_then(|r| r.total_cost_usd);
        let _ = store::end_run(&lock(&self.db), run, code, err.as_deref(), cost, t0.elapsed().as_millis());
        att
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(&self, id: i64, turn: i64, step: Step, a: Attempt, project: &Path, before: &brief::Snapshot, d: &Dirs) {
        if a.stopped {
            let conn = lock(&self.db);
            let _ = store::mark_interrupted(&conn, id, turn);
            let _ = store::add_message(&conn, id, turn, "note", NOTE_STOPPED, None);
            let _ = store::set_state(&conn, id, "stopped");
            return;
        }
        if !a.success() {
            self.fail(id, turn, &a.error_text());
            return;
        }
        let next = {
            let conn = lock(&self.db);
            let mut block: Option<Value> = None;
            if let Some((mid, text)) = &a.last_text {
                let an = p2g::analyse(text);
                if let Some(f) = an.blocks.iter().rev().find(|f| f.block.is_some()) {
                    block = serde_json::to_value(f.block.as_ref()).ok();
                } else if let Some(f) = an.blocks.first() {
                    let _ = store::add_message(
                        &conn,
                        id,
                        turn,
                        "error",
                        &format!(
                            "Claude sent a question block the app could not read: {}. Answer in the box below.",
                            f.errors.first().map(String::as_str).unwrap_or("unreadable block")
                        ),
                        None,
                    );
                }
                let _ = conn.execute(
                    "UPDATE grill_messages SET text = ?2, questions = ?3 WHERE id = ?1",
                    rusqlite::params![mid, an.prose, block.as_ref().map(Value::to_string)],
                );
            }
            // No p2g block shows as raw JSON in an earlier message of the same turn.
            let earlier: Vec<(i64, String)> = conn
                .prepare("SELECT id, text FROM grill_messages WHERE session_id = ?1 AND turn = ?2 AND role = 'claude'")
                .and_then(|mut s| s.query_map(rusqlite::params![id, turn], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
                .unwrap_or_default();
            for (mid, t) in earlier {
                if t.contains(p2g::TAG) {
                    let _ = conn.execute("UPDATE grill_messages SET text = ?2 WHERE id = ?1", rusqlite::params![mid, p2g::strip_blocks(&t)]);
                }
            }
            if let Some(b) = brief::changed_since(project, before) {
                let _ = store::add_brief_version(&conn, id, &b.path, &b.content, &b.sha);
            }
            let latest = store::latest_brief(&conn, id).ok().flatten();
            let next = next_state(block.as_ref(), latest.is_some(), latest.as_ref().is_some_and(|b| b.confirmed));
            let _ = conn.execute("UPDATE grill_sessions SET pending_message = NULL, error = NULL WHERE id = ?1", [id]);
            if step != Step::Adr {
                let _ = store::set_state(&conn, id, next);
            }
            next
        };
        if step == Step::Adr {
            // The session stays running while the app prepares the ADR; nothing is pushed before the
            // owner read it (security review finding 3).
            self.changed(id);
            let (note, review) = self.prepare_adr(id, project, d);
            let conn = lock(&self.db);
            let _ = store::add_message(&conn, id, turn, "note", &note, None);
            let _ = conn.execute(
                "UPDATE grill_sessions SET adr_pending = 0, adr_review = ?2, app_note = CASE WHEN ?2 IS NULL THEN ?3 ELSE NULL END WHERE id = ?1",
                rusqlite::params![id, review, note],
            );
            let _ = store::set_state(&conn, id, next);
        }
    }

    fn adr_log(&self, id: i64) -> impl FnMut(&super::adr::Step) + '_ {
        let db = self.db.clone();
        move |s: &super::adr::Step| {
            let conn = lock(&db);
            if let Ok(run) = store::start_run(&conn, id, "adr", &s.display, &s.argv) {
                let err = if s.out.ok() { None } else { Some(s.out.error_tail()) };
                let _ = store::end_run(&conn, run, s.out.code, err.as_deref(), None, s.out.ms);
            }
        }
    }

    /// (note, prepared ADR as JSON when one waits for the owner).
    fn prepare_adr(&self, id: i64, project: &Path, d: &Dirs) -> (String, Option<String>) {
        let tools = match super::adr::Tools::find(&self.home) {
            Ok(t) => t,
            Err(err) => return (super::adr::note(&Err(err)), None),
        };
        let mut log = self.adr_log(id);
        match super::adr::prepare(project, &d.scratch, &d.decisions_file, &tools, &mut log) {
            Ok(Some(p)) => (
                format!("ADR ready for you to read: {}. Nothing is pushed until you say yes.", p.files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>().join(", ")),
                serde_json::to_string(&p).ok(),
            ),
            Ok(None) => (super::adr::NOTE_NO_FILE.to_string(), None),
            Err(err) => (super::adr::note(&Err(err)), None),
        }
    }

    /// The owner read the prepared ADR: publish it (push `adr/<slug>`, open the PR) or discard it.
    /// The result is a note in the chat and is told to Claude at the start of the next message.
    pub fn adr_decide(&self, id: i64, publish: bool) -> Result<String, String> {
        let review: Option<String> = {
            let conn = lock(&self.db);
            self.check_free(&conn)?;
            conn.query_row("SELECT adr_review FROM grill_sessions WHERE id = ?1", [id], |r| r.get(0)).map_err(e)?
        };
        let p: super::adr::Prepared = serde_json::from_str(&review.ok_or("There is no ADR waiting for you.")?).map_err(e)?;
        let tools = super::adr::Tools::find(&self.home)?;
        let note = {
            let mut log = self.adr_log(id);
            if publish {
                super::adr::note(&super::adr::publish(&p, &tools, &mut log))
            } else {
                super::adr::discard(&p, &tools, &mut log);
                "ADR not published: you chose not to push it. The ADR file was removed.".to_string()
            }
        };
        let conn = lock(&self.db);
        let turn = store::turn_context(&conn, id)?.turns;
        store::add_message(&conn, id, turn, "note", &note, None).map_err(e)?;
        conn.execute("UPDATE grill_sessions SET adr_review = NULL, app_note = ?2 WHERE id = ?1", rusqlite::params![id, note]).map_err(e)?;
        drop(conn);
        self.changed(id);
        Ok(note)
    }
}


/// The session uuid is the scratch folder's name.
fn dirs_uuid(d: &Dirs) -> String {
    d.scratch.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Writes the version's HTML to `<scratch>/proto/prototype.html` (0700 folders, atomic), before every
/// turn, so a temp cleanup between turns does no harm. The name is fixed, never the artifact's.
fn write_prototype(d: &Dirs, html: &str) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    for dir in [d.scratch.parent().unwrap_or(&d.scratch), &d.scratch, &d.proto, &d.decisions] {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let tmp = d.proto.join(".prototype.html.tmp");
    std::fs::write(&tmp, html)?;
    std::fs::rename(&tmp, d.proto.join("prototype.html"))
}

struct Launch {
    id: i64,
    turn: i64,
    message: String,
    step: Step,
    tc: TurnContext,
}

struct Attempt {
    code: Option<i32>,
    result: Option<TurnResult>,
    stderr: String,
    stopped: bool,
    timed_out: bool,
    bad_mode: Option<String>,
    last_text: Option<(i64, String)>,
    spawn_error: Option<String>,
}

impl Attempt {
    fn error(msg: String) -> Attempt {
        Attempt { code: None, result: None, stderr: String::new(), stopped: false, timed_out: false, bad_mode: None, last_text: None, spawn_error: Some(msg) }
    }

    fn success(&self) -> bool {
        self.spawn_error.is_none()
            && !self.stopped
            && !self.timed_out
            && self.bad_mode.is_none()
            && self.code == Some(0)
            && self.result.as_ref().is_some_and(TurnResult::ok)
    }

    /// The real error, unchanged where Claude gave one (HANDOFF §8: show the real error).
    fn error_text(&self) -> String {
        if let Some(m) = &self.spawn_error {
            return m.clone();
        }
        if self.timed_out {
            return "Claude did not finish this turn in 15 minutes, so the app stopped it. Press Resume to try again.".into();
        }
        if let Some(m) = &self.bad_mode {
            return format!("Claude started in permission mode \"{m}\" instead of dontAsk, so the app stopped it.");
        }
        if let Some(r) = &self.result {
            if !r.ok() && !r.result.trim().is_empty() {
                return r.result.trim().to_string();
            }
        }
        let tail: Vec<&str> = self.stderr.trim().lines().collect();
        if !tail.is_empty() {
            return tail[tail.len().saturating_sub(8)..].join("\n");
        }
        match (&self.result, self.code) {
            (Some(r), _) if !r.ok() => format!("Claude ended the turn with \"{}\".", r.subtype),
            (None, Some(0)) => "Claude ended without a result.".into(),
            (_, Some(c)) => format!("claude exited with code {c}."),
            (_, None) => "claude was stopped by a signal.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_v4() {
        let u = new_uuid().unwrap();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
        assert!("89ab".contains(&u[19..20]));
        assert_ne!(u, new_uuid().unwrap());
    }

    #[test]
    fn the_adr_yes_fails_closed() {
        for y in ["Yes", "yes, make the ADR", "Make an ADR", "YES"] {
            assert!(is_adr_yes(y), "{y}");
        }
        for n in ["No, brief only", "Brief only", "Skip", "Yesterday's idea", "Maker", "not yes"] {
            assert!(!is_adr_yes(n), "{n}");
        }
    }

    fn block(header: &str, q: &str) -> Value {
        json!({"questions": [{"question": q, "header": header, "multiSelect": false,
            "options": [{"label": "Yes", "description": ""}, {"label": "No", "description": ""}]}]})
    }

    #[test]
    fn the_end_of_turn_state_table() {
        let other = block("Scope", "Which methods?");
        let brief_q = block("Brief", "Does this brief say what you mean?");
        assert_eq!(next_state(Some(&other), false, false), "waiting_owner");
        assert_eq!(next_state(Some(&brief_q), false, false), "waiting_owner", "no brief file: nothing to confirm");
        assert_eq!(next_state(Some(&brief_q), true, false), "brief_ready");
        assert_eq!(next_state(Some(&other), true, false), "waiting_owner");
        assert_eq!(next_state(Some(&block("ADR", "Make X an ADR?")), true, true), "waiting_owner");
        assert_eq!(next_state(None, true, true), "confirmed");
        assert_eq!(next_state(None, true, false), "brief_ready");
        assert_eq!(next_state(None, false, false), "waiting_owner");
    }

    #[test]
    fn an_interrupted_answer_is_wrapped_and_a_first_command_is_sent_again() {
        let first = format!("{} (Mode: interactive) on the prototype /t/prototype.html.", flags::FIRST_PREFIX);
        assert_eq!(resume_message(Some(&first)), first);
        let m = resume_message(Some("{\"answers\":{\"Q?\":\"A\"}}"));
        assert!(m.starts_with("The last turn was interrupted"));
        assert!(m.contains("{\"answers\":{\"Q?\":\"A\"}}"));
        assert!(m.contains("If you already acted on it"));
        assert_eq!(resume_message(None), "Continue.");
    }

    #[test]
    fn errors_show_claudes_own_text() {
        let mut a = Attempt::error("x".into());
        a.spawn_error = None;
        a.code = Some(1);
        a.result = Some(TurnResult { subtype: "success".into(), is_error: true, result: "Claude AI usage limit reached|1764000000".into(), ..Default::default() });
        assert!(!a.success());
        assert_eq!(a.error_text(), "Claude AI usage limit reached|1764000000");
        a.result = None;
        a.stderr = "line1\nError: rate limited".into();
        assert_eq!(a.error_text(), "line1\nError: rate limited");
    }
}
