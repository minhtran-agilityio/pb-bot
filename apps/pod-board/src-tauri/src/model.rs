//! The page model, built from raw `podbay-dev` JSON and the local session files (`local.rs`).
//! It copies named fields only (an allow list): account, uuid, dir, path, remote_url, cwd and the Claude session id
//! never leave this module. The only id that does is the Remote Control id (`rc_id`).

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

const WHAT_CHARS: usize = 160;
const NAME_CHARS: usize = 80;
/// At most this many links of one session reach the page (pull requests first, then issues, then artifacts).
const MAX_LINKS: usize = 8;
const URL_CHARS: usize = 400;

/// A page a session made or works on, as Pod Bay lists it under `links`: a pull request, an issue or an artifact.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Link {
    /// "pr", "issue" or "artifact"
    pub kind: &'static str,
    pub title: String,
    /// Always an `https://` address (see `safe_url`).
    pub url: String,
}

/// Why a waiting session waits, when its file says. Only "permission prompt" is told apart today (step 0, 2026-11-25):
/// a question and "input needed" read the same, and no file holds the command, so `detail` is empty.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Reason {
    /// "permission", "question" or "plan"
    pub kind: &'static str,
    pub detail: String,
}

/// One sub-agent of a session on this machine.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Agent {
    /// `agentType`, for example "Explore"
    #[serde(rename = "type")]
    pub kind: String,
    /// `description`
    pub task: String,
    pub model: String,
    /// When it started (ms since the epoch), from its transcript's file time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_s: Option<u64>,
    /// 0: a child of the main session; 1: a child of a sub-agent.
    pub depth: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Agents {
    pub running: usize,
    pub done: usize,
    pub list: Vec<Agent>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Session {
    /// Where it runs: "podbay" (listed by Pod Bay), "terminal" (a terminal session on this machine with Remote
    /// Control on, which Pod Bay does not list) or "desktop" (a session of the Claude desktop app, issue #176).
    pub source: &'static str,
    pub machine: String,
    pub project: String,
    pub ws: String,
    pub title: String,
    /// "needs", "busy" or "idle"
    pub state: &'static str,
    pub since: i64,
    pub what: String,
    pub branch: String,
    pub prs: usize,
    pub queued: usize,
    /// What the session waits for, as Pod Bay words it (`waiting_for`, for example "input needed"); else empty.
    pub waiting: String,
    /// The session's Remote Control id: the last part of `remote_url` (`https://claude.ai/code/<id>`), else empty.
    pub rc_id: String,
    /// A desktop session's id for the desktop app (`local_<uuid>`, the `sessionId` of its file): the one id of a
    /// desktop session that reaches the page, because the app needs it to open the session. Empty for any other source.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub desktop_id: String,
    /// Pod Bay's name for the session (`name`, for example `claude-pb-bot-00`), the argument of `podbay-dev detail`.
    /// Empty when it is not a safe name (see `safe_name`).
    pub name: String,
    /// The session's pull requests, issues and artifacts, each with its address.
    pub links: Vec<Link>,
    /// When the session finished a long run and went idle (ms since the epoch), until the owner looks at it; else 0.
    /// `build` leaves it 0: `Alerts::mark` sets it, because it depends on what earlier reads showed.
    pub finished: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// Sub-agents, for a session on this machine that has any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Agents>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Stopped {
    /// "" for a workspace with no session; "desktop" for a desktop-app session that closed lately (issue #176).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub source: &'static str,
    pub machine: String,
    pub project: String,
    pub ws: String,
    pub dirty: bool,
    /// A closed desktop session's title and when it closed (ms since the epoch); empty and 0 otherwise.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub closed: u64,
}

fn is_zero(n: &u64) -> bool { *n == 0 }

/// A desktop session with no process whose last activity was less than this long ago is in the Stopped strip; an older
/// one is not shown.
pub const CLOSED_MS: u64 = 12 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub online: bool,
    /// This machine's Pod Bay holds an out-of-date password for it (`password_current: false`): the two cannot link.
    pub key_stale: bool,
    /// The app gets "No route to host" from this machine's local network address (issue #174). The cause can be
    /// macOS Local Network access for Pod Board or the host being off.
    pub unreachable: bool,
    /// Pod Bay gave no address for this machine and `ls --all` returned no rows for it (issue #177): the app has
    /// nothing to probe, so it cannot say why. The cause can be the host being off, an out-of-date password or macOS
    /// Local Network access for Pod Board.
    pub no_address: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Counts {
    pub needs: usize,
    pub busy: usize,
    pub idle: usize,
    pub stopped: usize,
    /// Idle sessions that finished a run the owner has not looked at yet (they also count in `idle`).
    pub finished: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Model {
    pub machines: Vec<Machine>,
    pub sessions: Vec<Session>,
    pub stopped: Vec<Stopped>,
    pub counts: Counts,
    /// Machines `ls --all` named but could not read this time (their `result` is not a list). Their sessions are
    /// missing from `sessions` only for now, so alerts keep what they knew about them.
    #[serde(skip)]
    pub unread: Vec<String>,
    /// Built from a failed `ls` (`Raw::stale`): the Pod Bay part is the last read.
    #[serde(skip)]
    pub stale: bool,
}

/// Raw output of the read-only commands. A value that was never read is `Value::Null`.
#[derive(Debug, Clone, Default)]
pub struct Raw {
    pub ls: Value,
    pub checkouts: Value,
    pub machines: Value,
    pub queue: Value,
    /// The Claude sessions of this machine (`local::read_with`).
    pub local: Vec<crate::local::Local>,
    /// The sessions of the Claude desktop app on this machine (`local::read_desktop`).
    pub desktop: Vec<crate::local::Desktop>,
    /// The time of the read (ms since the epoch). 0 when not set: then no desktop session counts as closed lately.
    pub now: u64,
    /// Machines (by name) the app gets "No route to host" from (`unreachable_machines` in main.rs).
    pub unreachable: Vec<String>,
    /// The last `ls --all` failed: `ls` holds the read before it, so every machine in it counts as not read.
    pub stale: bool,
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// `context` is "<project>/<repo>" for src and "<project>/<repo>@NN" for src-NN.
pub fn ws_of(context: &str) -> String {
    match context.rsplit_once('@') {
        Some((_, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => format!("src-{n}"),
        _ => "src".to_string(),
    }
}

/// `[{machine, result: [...]}]` as (machine, rows). A machine whose result is not a list is skipped.
fn per_machine(v: &Value) -> Vec<(String, &Vec<Value>)> {
    let mut out = Vec::new();
    if let Some(list) = v.as_array() {
        for m in list {
            if let Some(rows) = m.get("result").and_then(Value::as_array) {
                out.push((text(&m["machine"]), rows));
            }
        }
    }
    out
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// A session needs a human when Pod Bay says so in any of its ways:
/// - `waiting_for` names what it waits for (seen: "input needed", with `status: "waiting"` and `asks: false`);
/// - `status` is anything other than "busy" or "idle" (seen: "waiting");
/// - `asks` is true (Pod Bay's guess that the last reply ends with a question);
/// - no `status` yet: a session stopped before its prompt, for example at a startup dialog.
fn state_of(s: &Value) -> &'static str {
    let status = s["status"].as_str();
    let waits = s["waiting_for"].as_str().map_or(false, |w| !w.trim().is_empty());
    if waits || truthy(&s["asks"]) || !matches!(status, Some("busy") | Some("idle")) {
        "needs"
    } else if status == Some("busy") {
        "busy"
    } else {
        "idle"
    }
}

/// The reason behind a `waitingFor` / `waiting_for` text, when it names one.
pub fn reason_of(waiting_for: &str) -> Option<Reason> {
    match waiting_for.trim() {
        "permission prompt" => Some(Reason { kind: "permission", detail: String::new() }),
        _ => None,
    }
}

/// The Remote Control id of a session, from its link `https://claude.ai/code/<id>` (ids look like `session_01…` or
/// `cshk_01…`). Only letters, digits and `_` pass, so the id is safe to put in a `claude://code/<id>` link.
pub fn session_id(raw: &str) -> String {
    let id = raw.strip_prefix("https://claude.ai/code/").unwrap_or("");
    let ok = !id.is_empty() && id.len() <= 128 && id.contains('_') && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok { id.to_string() } else { String::new() }
}

/// A Pod Bay session or machine name that is safe to pass to `podbay-dev` as one argument: letters, digits, `-`,
/// `_` and `.`, not starting with `-` (so it cannot read as a flag), at most 128 characters. Anything else is empty.
pub fn safe_name(raw: &str) -> String {
    let ok = !raw.is_empty()
        && raw.len() <= 128
        && !raw.starts_with('-')
        && raw.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok { raw.to_string() } else { String::new() }
}

/// A link that is safe to hand to the system's `open`: `https://` only, printable ASCII with no quote, bracket or
/// backslash, and not too long. Anything else is empty.
pub fn safe_url(raw: &str) -> String {
    let ok = raw.len() <= URL_CHARS
        && raw.strip_prefix("https://").map_or(false, |rest| rest.chars().next().map_or(false, |c| c.is_ascii_alphanumeric()))
        && raw.chars().all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\'' | '<' | '>' | '\\' | '^' | '`' | '{' | '|' | '}'));
    if ok { raw.to_string() } else { String::new() }
}

/// The links of one session, pull requests first. A link with no safe address is left out.
fn links_of(s: &Value) -> Vec<Link> {
    let mut out = Vec::new();
    for (field, kind) in [("prs", "pr"), ("issues", "issue"), ("artifacts", "artifact")] {
        for l in s["links"][field].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let url = safe_url(l["url"].as_str().unwrap_or(""));
            if !url.is_empty() && out.len() < MAX_LINKS {
                out.push(Link { kind, title: cut(l["title"].as_str().unwrap_or("").trim(), NAME_CHARS), url });
            }
        }
    }
    out
}

fn rank(state: &str) -> u8 {
    match state {
        "needs" => 0,
        "busy" => 1,
        _ => 2,
    }
}

/// Needs-you and busy: the longest wait first. Idle: the finished ones first (the latest on top), then the most recent.
pub fn order(sessions: &mut [Session]) {
    sessions.sort_by(|a, b| {
        rank(a.state)
            .cmp(&rank(b.state))
            .then_with(|| b.finished.cmp(&a.finished))
            .then_with(|| if a.state == "idle" { b.since.cmp(&a.since) } else { a.since.cmp(&b.since) })
            .then_with(|| a.title.cmp(&b.title))
    });
}

pub fn build(raw: &Raw) -> Model {
    let mut queued: HashMap<String, usize> = HashMap::new();
    for (machine, rows) in per_machine(&raw.queue) {
        for q in rows {
            // Only messages still in the line: `unknown` and `sending` entries also have no `finished`.
            let waits = match q["state"].as_str() {
                Some(state) => state == "queued",
                None => q["finished"].is_null(),
            };
            let dir = text(&q["dir"]);
            if waits && !dir.is_empty() {
                *queued.entry(format!("{machine}|{dir}")).or_insert(0) += 1;
            }
        }
    }

    let mut branch: HashMap<String, String> = HashMap::new();
    let mut seen: BTreeMap<(String, String, String), bool> = BTreeMap::new();
    for (machine, rows) in per_machine(&raw.checkouts) {
        for c in rows {
            let (project, ws) = (text(&c["project"]), text(&c["ws"]));
            if project.is_empty() || ws.is_empty() {
                continue;
            }
            branch.insert(format!("{machine}|{project}|{}|{ws}", text(&c["repo"])), text(&c["branch"]));
            let dirty = seen.entry((project, ws, machine.clone())).or_insert(false);
            *dirty = *dirty || truthy(&c["dirty"]);
        }
    }

    let mut sessions = Vec::new();
    for (machine, rows) in per_machine(&raw.ls) {
        for s in rows {
            if !s.is_object() {
                continue;
            }
            let project = text(&s["project"]);
            let ws = ws_of(&text(&s["context"]));
            seen.remove(&(project.clone(), ws.clone(), machine.clone()));
            let title = if truthy(&s["title"]) { text(&s["title"]) } else { text(&s["name"]) };
            sessions.push(Session {
                branch: cut(branch.get(&format!("{machine}|{project}|{}|{ws}", text(&s["repo"]))).map(String::as_str).unwrap_or(""), NAME_CHARS),
                queued: queued.get(&format!("{machine}|{}", text(&s["dir"]))).copied().unwrap_or(0),
                rc_id: session_id(&text(&s["remote_url"])),
                desktop_id: String::new(),
                name: safe_name(&text(&s["name"])),
                waiting: cut(s["waiting_for"].as_str().unwrap_or("").trim(), 40),
                reason: reason_of(s["waiting_for"].as_str().unwrap_or("")),
                agents: None,
                source: "podbay",
                prs: s["links"]["prs"].as_array().map(Vec::len).unwrap_or(0),
                links: links_of(s),
                finished: 0,
                state: state_of(s),
                since: s["since"].as_i64().unwrap_or(0),
                what: cut(&text(&s["description"]), WHAT_CHARS),
                title: cut(&title, NAME_CHARS),
                machine: machine.clone(),
                project,
                ws,
            });
        }
    }
    let mut unread: Vec<String> = raw.ls.as_array().map(|list| {
        list.iter().filter(|m| (raw.stale || !m["result"].is_array()) && truthy(&m["machine"])).map(|m| text(&m["machine"])).collect()
    }).unwrap_or_default();
    let me = me(&raw.machines);
    if raw.stale && !me.is_empty() && !unread.contains(&me) { unread.push(me.clone()); }
    let shown = merge_local(&mut sessions, &raw.local, &me, &unread);
    let mut closed = merge_desktop(&mut sessions, raw, &shown, &me);
    order(&mut sessions);

    let mut stopped: Vec<Stopped> = seen
        .into_iter()
        .map(|((project, ws, machine), dirty)| Stopped { source: "", machine, project, ws, dirty, title: String::new(), closed: 0 })
        .collect();
    // The latest closed first.
    closed.sort_by(|a, b| b.closed.cmp(&a.closed).then_with(|| a.title.cmp(&b.title)));
    stopped.extend(closed);
    let ls_read = !raw.stale && raw.ls.is_array();
    let read_rows: Vec<String> = raw.ls.as_array().map(|list| {
        list.iter().filter(|m| m["result"].is_array() && truthy(&m["machine"])).map(|m| text(&m["machine"])).collect()
    }).unwrap_or_default();
    let machines = raw
        .machines
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|m| truthy(&m["machine"]))
                .map(|m| {
                    let name = text(&m["machine"]);
                    // Only a fresh `ls` can say a machine returned no rows: before the first read, or after a failed one, it cannot.
                    let no_rows = ls_read && !read_rows.contains(&name);
                    let no_address = no_rows && m["me"] != Value::Bool(true) && !truthy(&m["addr"]);
                    Machine { online: truthy(&m["online"]), key_stale: m["password_current"] == Value::Bool(false), unreachable: raw.unreachable.contains(&name), no_address, name }
                })
                .collect()
        })
        .unwrap_or_default();

    let mut counts = Counts { stopped: stopped.len(), ..Counts::default() };
    for s in &sessions {
        match s.state {
            "needs" => counts.needs += 1,
            "busy" => counts.busy += 1,
            _ => counts.idle += 1,
        }
    }
    Model { machines, sessions, stopped, counts, unread, stale: raw.stale }
}

/// The name of this machine in Pod Bay (`me: true` in `podbay-dev machines`), else empty.
fn me(machines: &Value) -> String {
    machines.as_array().and_then(|l| l.iter().find(|m| m["me"] == Value::Bool(true))).map(|m| text(&m["machine"])).unwrap_or_default()
}

/// Add the local sessions. One that Pod Bay also lists (the same tmux session on this machine, or the same Remote
/// Control id) shows once, as Pod Bay's, and lends it its sub-agents. Another one shows as "terminal" when it is an
/// interactive `cli` session with Remote Control on; any other one (a `claude -p` run, a session without Remote
/// Control, an entry point not seen in step 0) is left out. A local session in tmux is Pod Bay's to list: while this
/// machine's Pod Bay list is unknown or could not be read, it is left out rather than shown as a terminal session.
/// Returns the Claude session ids that now show on the board (a Pod Bay session that got a local twin, and each
/// terminal session added), so a desktop-app file for the same session does not make a second row.
fn merge_local(sessions: &mut Vec<Session>, local: &[crate::local::Local], me: &str, unread: &[String]) -> HashSet<String> {
    let pod_bay_known = !me.is_empty() && !unread.iter().any(|m| m == me);
    let mut shown = HashSet::new();
    for l in local {
        // Only an interactive `cli` session counts: a `claude -p` run inside a Pod Bay pane lends nothing.
        if l.entrypoint != "cli" || !l.interactive {
            continue;
        }
        let twin = sessions.iter_mut().find(|s| {
            s.source == "podbay"
                && ((!l.rc_id.is_empty() && s.rc_id == l.rc_id) || (!me.is_empty() && s.machine == me && !l.tmux.is_empty() && s.name == l.tmux))
        });
        if let Some(s) = twin {
            if s.agents.is_none() { s.agents = l.agents.clone(); }
            shown.insert(l.session_id.clone());
            continue;
        }
        if l.rc_id.is_empty() || (!l.tmux.is_empty() && !pod_bay_known) {
            // A tmux session is Pod Bay's to list: while Pod Bay's list is unknown it is not a terminal row, and no
            // desktop row either, or it would show as Desktop now and as Pod Bay later (issue #176 review F2).
            if !l.tmux.is_empty() && !pod_bay_known { shown.insert(l.session_id.clone()); }
            continue;
        }
        let waits = !l.waiting_for.trim().is_empty();
        let state = match l.status.as_str() {
            "busy" if !waits => "busy",
            "idle" if !waits => "idle",
            _ => "needs",
        };
        let title = if l.title.is_empty() { l.project.clone() } else { l.title.clone() };
        sessions.push(Session {
            source: "terminal",
            machine: me.to_string(),
            project: l.project.clone(),
            ws: String::new(),
            title: cut(&title, NAME_CHARS),
            state,
            since: l.since,
            what: String::new(),
            branch: String::new(),
            prs: 0,
            queued: 0,
            waiting: l.waiting_for.clone(),
            rc_id: l.rc_id.clone(),
            desktop_id: String::new(),
            name: String::new(),
            links: Vec::new(),
            finished: 0,
            reason: reason_of(&l.waiting_for),
            agents: l.agents.clone(),
        });
        shown.insert(l.session_id.clone());
    }
    shown.remove("");
    shown
}

/// Add the Claude desktop app's sessions (issue #176). The app's file has no status, so:
/// - a session that Pod Bay or a terminal row already shows (same Claude session id) is left out: it shows once, there;
/// - one with a live process (`~/.claude/sessions/<pid>.json` with the same session id, whatever its entry point) takes
///   its state, wait reason and sub-agents from that file, like a terminal session;
/// - one with no process is closed: when its last activity was within `CLOSED_MS` it is returned for the Stopped
///   strip; any older one, and any one when the read has no clock, is left out (owner, 2026-11-26: "hide it after
///   12 h"; what cannot be known is left out, no state is made up).
fn merge_desktop(sessions: &mut Vec<Session>, raw: &Raw, shown: &HashSet<String>, me: &str) -> Vec<Stopped> {
    let mut closed = Vec::new();
    for d in &raw.desktop {
        if !d.cli_id.is_empty() && shown.contains(&d.cli_id) {
            continue;
        }
        let title = if d.title.is_empty() { d.project.clone() } else { d.title.clone() };
        let live = (!d.cli_id.is_empty()).then(|| raw.local.iter().find(|l| l.session_id == d.cli_id)).flatten();
        let last = d.last_activity.max(0);
        let Some(l) = live else {
            if raw.now > 0 && last > 0 && (last as u64) <= raw.now && raw.now - (last as u64) < CLOSED_MS {
                closed.push(Stopped { source: "desktop", machine: me.to_string(), project: d.project.clone(), ws: String::new(), dirty: false, title: cut(&title, NAME_CHARS), closed: last as u64 });
                continue;
            }
            continue;
        };
        let waits = !l.waiting_for.trim().is_empty();
        let state = match l.status.as_str() {
            "busy" if !waits => "busy",
            "idle" if !waits => "idle",
            _ => "needs",
        };
        sessions.push(desktop_row(d, &title, me, state, l.since, &l.waiting_for, reason_of(&l.waiting_for), l.agents.clone()));
    }
    closed
}

#[allow(clippy::too_many_arguments)]
fn desktop_row(d: &crate::local::Desktop, title: &str, me: &str, state: &'static str, since: i64, waiting: &str, reason: Option<Reason>, agents: Option<Agents>) -> Session {
    Session {
        source: "desktop",
        machine: me.to_string(),
        project: d.project.clone(),
        ws: String::new(),
        title: cut(title, NAME_CHARS),
        state,
        since,
        what: String::new(),
        branch: String::new(),
        prs: 0,
        queued: 0,
        waiting: waiting.to_string(),
        rc_id: String::new(),
        desktop_id: d.open_id.clone(),
        name: String::new(),
        links: Vec::new(),
        finished: 0,
        reason,
        agents,
    }
}

/// One session's identity across reads: its Remote Control id when it has one (stable when the title changes),
/// else machine, workspace and title together.
pub fn key(s: &Session) -> String {
    if !s.desktop_id.is_empty() {
        format!("desktop|{}", s.desktop_id)
    } else if s.rc_id.is_empty() { format!("{}|{}/{}|{}", s.machine, s.project, s.ws, s.title) } else { s.rc_id.clone() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_machine_with_no_route_from_the_app_is_marked() {
        let r = Raw {
            machines: json!([
                {"machine": "M0", "me": true, "online": true, "password_current": true},
                {"machine": "U0", "me": false, "online": false, "password_current": true, "addr": "192.168.1.218:7880"},
                {"machine": "UA", "me": false, "online": false, "password_current": false}]),
            unreachable: vec!["U0".to_string()],
            ..Raw::default()
        };
        let got: Vec<(String, bool)> = build(&r).machines.into_iter().map(|m| (m.name, m.unreachable)).collect();
        assert_eq!(got, vec![("M0".to_string(), false), ("U0".to_string(), true), ("UA".to_string(), false)]);
    }

    #[test]
    fn a_machine_that_gave_no_rows_and_no_address_is_marked_not_reachable() {
        // Issue #177: with Local Network off for the app, Pod Bay lists U0 offline with no address, so the app cannot
        // probe it and the #174 mark never shows. No rows and no address is what the app can say plainly.
        let r = Raw {
            ls: json!([
                {"machine": "M0", "result": []},
                {"machine": "U0", "result": "unreachable"},
                {"machine": "UC", "result": []}]),
            machines: json!([
                {"machine": "M0", "me": true, "online": true, "password_current": true},
                {"machine": "U0", "me": false, "online": false, "password_current": false},
                {"machine": "UB", "me": false, "online": false, "password_current": true},
                {"machine": "UC", "me": false, "online": true, "password_current": true},
                {"machine": "UD", "me": false, "online": false, "password_current": true, "addr": "192.168.1.5:7880"}]),
            unreachable: vec!["UD".to_string()],
            ..Raw::default()
        };
        let got: Vec<(String, bool, bool)> = build(&r).machines.into_iter().map(|m| (m.name, m.no_address, m.unreachable)).collect();
        assert_eq!(got, vec![
            ("M0".to_string(), false, false),
            ("U0".to_string(), true, false),  // named by ls but not read, no address to try
            ("UB".to_string(), true, false),  // not in ls at all, no address to try
            ("UC".to_string(), false, false), // returned rows: it was reached
            ("UD".to_string(), false, true),  // has an address: the probe decides (#174)
        ]);
        // Before the first `ls` read nothing is known about any machine: no mark.
        let none = Raw { machines: r.machines.clone(), ..Raw::default() };
        assert!(build(&none).machines.iter().all(|m| !m.no_address));
        // After a failed `ls` the rows are the last read, not this one: no mark from them either.
        let stale = Raw { stale: true, ..r.clone() };
        assert!(build(&stale).machines.iter().all(|m| !m.no_address));
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn only_a_plain_name_passes_to_podbay_dev() {
        for ok in ["claude-pb-bot-00", "M0", "U0", "mnemo_fd.2"] { assert_eq!(safe_name(ok), ok); }
        for bad in ["", "-rf", "--on", "a b", "a;b", "a/b", "../x", "x\n"] { assert_eq!(safe_name(bad), "", "{bad:?}"); }
        assert_eq!(safe_name(&"a".repeat(129)), "");
    }

    fn sess(over: Value) -> Value {
        let mut base = json!({
            "title": "T", "name": "n", "uuid": "UUID-SECRET", "project": "mnemo", "repo": "mnemo", "context": "mnemo/mnemo",
            "dir": "/home/someone/secret/dir", "status": "idle", "asks": false, "since": 1000, "description": null,
            "links": {"prs": [], "issues": [], "artifacts": []}, "account": "someone@example.com",
            "remote_url": "https://claude.ai/code/session_01AbC"
        });
        for (k, v) in over.as_object().unwrap() {
            base[k] = v.clone();
        }
        base
    }

    fn raw() -> Raw {
        Raw {
            ls: json!([
                {"machine": "UA", "result": [
                    sess(json!({"title": "Idle old", "context": "mnemo/mnemo@02", "since": 1000, "dir": "/d/02"})),
                    sess(json!({"title": "Idle new", "context": "mnemo/mnemo@03", "since": 9000})),
                    sess(json!({"title": "Busy", "context": "pb-bot/pb-bot@06", "project": "pb-bot", "repo": "pb-bot", "status": "busy", "since": 5000, "description": "board work"})),
                    sess(json!({"title": "Asks late", "context": "mnemo/mnemo@01", "asks": true, "since": 4000, "links": {"prs": [{"url": "u1"}, {"url": "u2"}]}})),
                    sess(json!({"title": "Asks early", "asks": true, "status": "busy", "since": 2000})),
                    sess(json!({"title": "Starting", "context": "mnemo/mnemo@04", "status": null, "since": 3000})),
                ]},
                {"machine": "M0", "result": []},
                {"machine": "Bad", "result": "unreachable"}
            ]),
            checkouts: json!([
                {"machine": "UA", "result": [
                    {"project": "mnemo", "repo": "mnemo", "ws": "src", "branch": "main", "dirty": 0, "path": "/home/someone/secret/path"},
                    {"project": "mnemo", "repo": "mnemo", "ws": "src-01", "branch": "feat/x", "dirty": 0},
                    {"project": "mnemo", "repo": "mnemo", "ws": "src-07", "branch": "main", "dirty": 0},
                    {"project": "mnemo", "repo": "extra", "ws": "src-07", "branch": null, "dirty": 1}
                ]},
                {"machine": "U0", "result": [{"project": "harness", "repo": "agent-harness", "ws": "src", "branch": "main", "dirty": 0}]}
            ]),
            machines: json!([{"machine": "UA", "online": true, "id": "ID-SECRET", "addr": "10.0.0.1:7880", "password_current": true}, {"machine": "Old", "online": false, "password_current": false}]),
            queue: json!([{"machine": "UA", "result": [
                {"dir": "/d/02", "finished": null}, {"dir": "/d/02", "finished": 123},
                {"dir": "/d/02", "state": "unknown", "finished": null}, {"dir": "/d/02", "state": "delivered", "finished": 5}
            ]}]),
            local: vec![],
            desktop: vec![],
            now: 0,
            unreachable: vec![],
            stale: false,
        }
    }

    #[test]
    fn only_claude_session_ids_are_kept() {
        assert_eq!(session_id("https://claude.ai/code/session_01MjxNHe8J1MMGhFig4bVsHc"), "session_01MjxNHe8J1MMGhFig4bVsHc");
        assert_eq!(session_id("https://claude.ai/code/cshk_01MjxNHektRUhVF7rY6wvoZ2YZha6dkw23rTzKhMV3iq7S"), "cshk_01MjxNHektRUhVF7rY6wvoZ2YZha6dkw23rTzKhMV3iq7S");
        for bad in ["", "git@example.com:o/r.git", "https://evil.example/code/session_1", "https://claude.ai/code/", "https://claude.ai/code/plain",
                    "https://claude.ai/code/session_1?x=y", "http://claude.ai/code/session_1", "https://claude.ai/code/session_1/../x", "https://claude.ai/code/a_b%20c"] {
            assert_eq!(session_id(bad), "", "{bad} was kept");
        }
        assert_eq!(build(&raw()).sessions[0].rc_id, "session_01AbC");
    }

    #[test]
    fn a_waiting_session_needs_you_even_without_asks() {
        let r = Raw {
            ls: json!([{"machine": "UA", "result": [
                sess(json!({"title": "Waiting", "status": "waiting", "asks": false, "waiting_for": "input needed"})),
                sess(json!({"title": "Odd status", "status": "paused", "asks": false})),
                sess(json!({"title": "Blank wait", "status": "busy", "asks": false, "waiting_for": "  "})),
                sess(json!({"title": "Plain idle", "status": "idle", "asks": false, "waiting_for": null})),
            ]}]),
            ..Raw::default()
        };
        let m = build(&r);
        let got: Vec<(&str, &str, &str)> = m.sessions.iter().map(|s| (s.title.as_str(), s.state, s.waiting.as_str())).collect();
        assert_eq!(got, vec![
            ("Odd status", "needs", ""), ("Waiting", "needs", "input needed"),
            ("Blank wait", "busy", ""), ("Plain idle", "idle", ""),
        ]);
    }

    #[test]
    fn ws_of_reads_the_workspace_number() {
        assert_eq!(ws_of("mnemo/mnemo"), "src");
        assert_eq!(ws_of("mnemo/mnemo@05"), "src-05");
        assert_eq!(ws_of("a@b/c"), "src");
        assert_eq!(ws_of(""), "src");
    }

    #[test]
    fn order_is_needs_then_busy_then_idle() {
        let m = build(&raw());
        let got: Vec<(&str, &str)> = m.sessions.iter().map(|s| (s.title.as_str(), s.state)).collect();
        assert_eq!(got, vec![
            ("Asks early", "needs"), ("Starting", "needs"), ("Asks late", "needs"),
            ("Busy", "busy"), ("Idle new", "idle"), ("Idle old", "idle"),
        ]);
        assert_eq!(m.counts, Counts { needs: 3, busy: 1, idle: 2, stopped: 2, finished: 0 });
    }

    #[test]
    fn fields_come_from_the_right_place() {
        let m = build(&raw());
        let by = |t: &str| m.sessions.iter().find(|s| s.title == t).unwrap().clone();
        assert_eq!(by("Asks late").ws, "src-01");
        assert_eq!(by("Asks late").branch, "feat/x");
        assert_eq!(by("Asks late").what, "");
        assert_eq!(by("Asks late").prs, 2);
        assert_eq!(by("Busy").what, "board work");
        assert_eq!(by("Idle old").queued, 1);
        assert_eq!(by("Idle new").queued, 0);
    }

    #[test]
    fn links_keep_their_kind_title_and_https_address() {
        let r = Raw { ls: json!([{"machine": "UA", "result": [sess(json!({"links": {
            "prs": [{"title": "o/r#266", "url": "https://github.com/o/r/pull/266"}, {"title": "bad", "url": "file:///etc/passwd"}],
            "issues": [{"title": "o/r#7", "url": "https://github.com/o/r/issues/7"}],
            "artifacts": [{"title": "Cost by version", "url": "https://claude.ai/artifact/3VE25w"}, {"url": "https://x.example/a b"}]
        }}))]}]), ..Raw::default() };
        let s = &build(&r).sessions[0];
        let got: Vec<(&str, &str, &str)> = s.links.iter().map(|l| (l.kind, l.title.as_str(), l.url.as_str())).collect();
        assert_eq!(got, vec![
            ("pr", "o/r#266", "https://github.com/o/r/pull/266"),
            ("issue", "o/r#7", "https://github.com/o/r/issues/7"),
            ("artifact", "Cost by version", "https://claude.ai/artifact/3VE25w"),
        ]);
        assert_eq!(s.prs, 2, "the count still says what Pod Bay lists");
    }

    #[test]
    fn only_a_plain_https_address_is_a_link() {
        for ok in ["https://github.com/o/r/pull/1", "https://claude.ai/artifact/aB3_-x?y=1&z=2#top"] { assert_eq!(safe_url(ok), ok); }
        for bad in ["", "http://github.com/o/r", "https://", "https:///x", "https://-x", "javascript:alert(1)", "file:///etc/passwd", "claude://code/x",
                    "https://a.example/a b", "https://a.example/\"x", "https://a.example/\n", "https://a.example/é", "-a https://a.example"] {
            assert_eq!(safe_url(bad), "", "{bad:?} was kept");
        }
        assert_eq!(safe_url(&format!("https://a.example/{}", "x".repeat(URL_CHARS))), "");
    }

    #[test]
    fn at_most_eight_links_reach_the_page() {
        let many: Vec<Value> = (0..20).map(|i| json!({"title": format!("o/r#{i}"), "url": format!("https://github.com/o/r/pull/{i}")})).collect();
        let r = Raw { ls: json!([{"machine": "UA", "result": [sess(json!({"links": {"prs": many}}))]}]), ..Raw::default() };
        assert_eq!(build(&r).sessions[0].links.len(), MAX_LINKS);
    }

    #[test]
    fn a_workspace_with_no_session_is_stopped_once() {
        let m = build(&raw());
        assert_eq!(m.stopped, vec![
            Stopped { source: "", title: String::new(), closed: 0, machine: "U0".into(), project: "harness".into(), ws: "src".into(), dirty: false },
            Stopped { source: "", title: String::new(), closed: 0, machine: "UA".into(), project: "mnemo".into(), ws: "src-07".into(), dirty: true },
        ]);
        assert_eq!(m.machines, vec![
            Machine { name: "UA".into(), online: true, key_stale: false, unreachable: false, no_address: false },
            Machine { name: "Old".into(), online: false, key_stale: true, unreachable: false, no_address: true },
        ]);
    }

    #[test]
    fn private_fields_never_reach_the_model() {
        let body = serde_json::to_string(&build(&raw())).unwrap();
        for secret in ["someone@example.com", "UUID-SECRET", "/home/someone", "ID-SECRET", "10.0.0.1"] {
            assert!(!body.contains(secret), "model holds {secret}");
        }
    }

    #[test]
    fn missing_or_odd_input_gives_an_empty_model() {
        assert_eq!(build(&Raw::default()), Model::default());
        let odd = Raw { ls: json!("x"), checkouts: json!([{"machine": "A", "result": "err"}]), machines: json!({}), queue: Value::Null, local: vec![], desktop: vec![], now: 0, unreachable: vec![], stale: false };
        assert_eq!(build(&odd), Model::default());
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        let long = "ồ".repeat(300);
        let r = Raw { ls: json!([{"machine": "UA", "result": [sess(json!({"description": long}))]}]), ..Raw::default() };
        let what = &build(&r).sessions[0].what;
        assert_eq!(what.chars().count(), WHAT_CHARS);
        assert!(what.ends_with('…'));
    }

    /// Pod Bay's view next to the local fixture folder (`tests/fixtures/local`): this machine is UA. Pod Bay lists the
    /// tmux session `claude-pb-bot-02` (the local "Pod Bay twin") and a session whose Remote Control id is the local
    /// "Remote twin"'s; M0 has a session with the same tmux name, which must not take the local one.
    fn with_local() -> Model {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        let r = Raw {
            ls: json!([
                {"machine": "UA", "result": [
                    sess(json!({"title": "Board work", "name": "claude-pb-bot-02", "project": "pb-bot", "context": "pb-bot/pb-bot@02", "remote_url": null})),
                    sess(json!({"title": "Twin by link", "name": "claude-pb-bot-05", "project": "pb-bot", "context": "pb-bot/pb-bot@05", "status": "busy",
                                "remote_url": "https://claude.ai/code/session_01FixtureTwin"})),
                    sess(json!({"title": "Plain wait", "name": "claude-pb-bot-06", "status": "waiting", "waiting_for": "input needed", "remote_url": null})),
                ]},
                {"machine": "M0", "result": [sess(json!({"title": "Same name elsewhere", "name": "claude-pb-bot-02", "remote_url": null}))]}
            ]),
            machines: json!([{"machine": "UA", "me": true, "online": true}, {"machine": "M0", "me": false, "online": true}]),
            local: crate::local::read(&crate::local::fixture(), now, &|_, _| true),
            ..Raw::default()
        };
        build(&r)
    }

    #[test]
    fn local_sessions_show_as_terminal_and_a_pod_bay_twin_shows_once() {
        let m = with_local();
        let mut got: Vec<(&str, &str, &str)> = m.sessions.iter().map(|s| (s.title.as_str(), s.source, s.machine.as_str())).collect();
        got.sort();
        assert_eq!(got, vec![
            ("Board work", "podbay", "UA"), ("Plain wait", "podbay", "UA"), ("Quiet terminal", "terminal", "UA"),
            ("Same name elsewhere", "podbay", "M0"), ("Sub-agent host", "terminal", "UA"), ("Twin by link", "podbay", "UA"),
            ("Waits on a push", "terminal", "UA"),
        ], "no row for the twins, the `claude -p` run or the session without Remote Control");
        let by = |t: &str| m.sessions.iter().find(|s| s.title == t).unwrap().clone();
        // The twin by tmux name lends its sub-agents to the Pod Bay row of this machine, not to M0's.
        assert_eq!(by("Board work").agents.as_ref().map(|a| (a.running, a.done)), Some((0, 1)));
        assert!(by("Same name elsewhere").agents.is_none());
        let t = by("Quiet terminal");
        assert_eq!((t.project.as_str(), t.rc_id.as_str(), t.state, t.name.as_str()), ("beta", "session_01FixtureQuiet", "idle", ""));
        assert_eq!(by("Sub-agent host").state, "busy");
        assert_eq!(m.counts, Counts { needs: 2, busy: 2, idle: 3, stopped: 0, finished: 0 });
    }

    #[test]
    fn a_print_run_in_a_pod_bay_pane_lends_nothing() {
        // Fixture 1011 is a `claude -p` run in the pane `claude-pb-bot-02` with one running sub-agent.
        let m = with_local();
        let b = m.sessions.iter().find(|s| s.title == "Board work").unwrap();
        assert_eq!(b.agents.as_ref().map(|a| (a.running, a.done)), Some((0, 1)), "only the interactive twin's sub-agents");
    }

    #[test]
    fn a_pod_bay_session_does_not_become_a_terminal_row_while_this_machine_is_unread() {
        let local = vec![crate::local::Local { tmux: "claude-pb-bot-02".into(), rc_id: "session_01InTmux".into(), entrypoint: "cli".into(),
            interactive: true, status: "busy".into(), title: "In tmux".into(), project: "pb-bot".into(), ..Default::default() },
            crate::local::Local { rc_id: "session_01Plain".into(), entrypoint: "cli".into(), interactive: true, status: "idle".into(),
            title: "Plain terminal".into(), project: "beta".into(), ..Default::default() }];
        let me = json!([{"machine": "UA", "me": true, "online": true}]);
        let titles = |ls: Value, machines: Value| -> Vec<String> {
            build(&Raw { ls, machines, local: local.clone(), ..Raw::default() }).sessions.iter().map(|s| s.title.clone()).collect()
        };
        assert_eq!(titles(json!([{"machine": "UA", "result": "unreachable"}]), me.clone()), vec!["Plain terminal"]);
        assert_eq!(titles(json!([{"machine": "UA", "result": []}]), Value::Null), vec!["Plain terminal"], "this machine unknown");
        let mut both = titles(json!([{"machine": "UA", "result": []}]), me);
        both.sort();
        assert_eq!(both, vec!["In tmux", "Plain terminal"], "a tmux session Pod Bay does not list shows when its list was read");
    }

    #[test]
    fn the_model_of_local_sessions_holds_no_path_account_or_session_id() {
        let m = with_local();
        let body = serde_json::to_string(&m).unwrap();
        for key in ["\"cwd\"", "\"dir\"", "\"path\"", "\"account\"", "\"uuid\"", "\"sessionId\"", "\"pid\"", "\"tmux\""] {
            assert!(!body.contains(key), "model has the field {key}");
        }
        for secret in ["/home/someone", "someone@example.com", "11111111-1111", "22222222-", "toolu_SECRET", "PRIVATE TRANSCRIPT", "/Users/"] {
            assert!(!body.contains(secret), "model holds {secret}");
        }
        let host = m.sessions.iter().find(|s| s.title == "Sub-agent host").unwrap();
        assert_eq!(host.project, "alpha", "the project is the last folder name only");
        let task = &host.agents.as_ref().unwrap().list.iter().find(|x| x.task.starts_with("Write tests")).unwrap().task;
        assert_eq!(task, "Write tests in ~/secret/alpha/tests", "a home path in a sub-agent's task loses the home folder");
    }

    #[test]
    fn sub_agents_reach_the_model_with_counts_and_depth() {
        let m = with_local();
        let a = m.sessions.iter().find(|s| s.title == "Sub-agent host").unwrap().agents.clone().unwrap();
        assert_eq!((a.running, a.done), (5, 2));
        assert_eq!(a.list.iter().find(|x| x.task == "Nested lookup").unwrap().depth, 1);
        assert!(m.sessions.iter().find(|s| s.title == "Quiet terminal").unwrap().agents.is_none());
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["list"][0]["type"], "Explore", "the page reads `type`");
    }

    #[test]
    fn a_known_wait_reason_has_its_kind_and_an_unknown_one_none() {
        let m = with_local();
        let by = |t: &str| m.sessions.iter().find(|s| s.title == t).unwrap().clone();
        let w = by("Waits on a push");
        assert_eq!((w.state, w.waiting.as_str()), ("needs", "permission prompt"));
        assert_eq!(w.reason, Some(Reason { kind: "permission", detail: String::new() }));
        let p = by("Plain wait");
        assert_eq!((p.state, p.waiting.as_str(), p.reason.clone()), ("needs", "input needed", None));
        let pb = build(&Raw { ls: json!([{"machine": "U0", "result": [sess(json!({"status": "waiting", "waiting_for": "permission prompt"}))]}]), ..Raw::default() });
        assert_eq!(pb.sessions[0].reason.as_ref().map(|r| r.kind), Some("permission"), "Pod Bay's own sessions on any machine too");
        assert!(serde_json::to_string(&by("Quiet terminal")).unwrap().find("\"reason\"").is_none(), "no reason: no field");
    }
    // ---- Desktop source (issue #176). Fixtures: tests/fixtures/desktop (its README says what each file must give). ----

    fn desktop_fixture() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desktop")
    }
    /// 1795000100000 is the `lastActivityAt` of every fixture file.
    const FIXTURE_ACTIVE: u64 = 1_795_000_100_000;

    /// A live process (a `~/.claude/sessions` file) for the desktop session called `title`.
    fn live_of(title: &str) -> crate::local::Local {
        let d = crate::local::read_desktop(&desktop_fixture()).into_iter().find(|d| d.title == title).expect(title);
        crate::local::Local { session_id: d.cli_id, entrypoint: "claude-desktop".into(), interactive: true, status: "idle".into(), since: 7, ..Default::default() }
    }

    const H: u64 = 60 * 60 * 1000;

    /// Every fixture file at once, with the local fixture (a terminal twin and a Pod Bay twin) and a Pod Bay row for
    /// the tmux session `claude-pb-bot-02`; with `live`, also a live process for `Index repo on desktop` and
    /// `Privacy check`. Read `after_ms` after the files' last activity.
    fn with_desktop(after_ms: u64, live: bool) -> Model {
        let mut local = crate::local::read(&crate::local::fixture(), FIXTURE_ACTIVE, &|_, _| true);
        if live { local.extend([live_of("Index repo on desktop"), live_of("Privacy check")]); }
        build(&Raw {
            ls: json!([{"machine": "UA", "result": [sess(json!({"title": "Pod Bay twin", "name": "claude-pb-bot-02", "context": "pb-bot/pb-bot@02", "project": "pb-bot"}))]}]),
            machines: json!([{"machine": "UA", "me": true, "online": true}]),
            local,
            desktop: crate::local::read_desktop(&desktop_fixture()),
            now: FIXTURE_ACTIVE + after_ms,
            ..Raw::default()
        })
    }

    /// (desktop session titles, desktop Stopped entry titles), sorted.
    fn desk_rows(m: &Model) -> (Vec<String>, Vec<String>) {
        let mut a: Vec<String> = m.sessions.iter().filter(|s| s.source == "desktop").map(|s| s.title.clone()).collect();
        let mut b: Vec<String> = m.stopped.iter().filter(|w| w.source == "desktop").map(|w| w.title.clone()).collect();
        a.sort();
        b.sort();
        (a, b)
    }

    #[test]
    fn the_desktop_fixture_gives_two_desktop_sessions_and_a_twin_shows_once_under_its_own_source() {
        // Case (a) of the fixtures README: live processes for d1 and d5.
        let m = with_desktop(13 * H, true);
        let mut desk: Vec<(&str, &str, &str)> = m.sessions.iter().filter(|s| s.source == "desktop").map(|s| (s.title.as_str(), s.project.as_str(), s.state)).collect();
        desk.sort();
        assert_eq!(desk, vec![("Index repo on desktop", "alpha", "idle"), ("Privacy check", "delta-desk", "idle")]);
        let by = |t: &str| m.sessions.iter().filter(|s| s.title == t).map(|s| s.source).collect::<Vec<_>>();
        assert_eq!(by("Remote twin"), vec!["terminal"], "the terminal twin shows once, as Terminal");
        assert_eq!(by("Pod Bay twin"), vec!["podbay"], "the Pod Bay twin shows once, as Pod Bay's");
        for gone in ["Archived desktop chat", "Desktop twin of a terminal session", "Desktop twin of a Pod Bay session"] {
            assert!(by(gone).is_empty(), "{gone} is not a session");
            assert!(m.stopped.iter().all(|w| w.title != gone), "{gone} is not a Stopped entry");
        }
        assert!(m.stopped.iter().all(|w| w.source != "desktop"), "the two live ones are not closed");
    }

    #[test]
    fn a_desktop_file_with_no_live_process_is_a_stopped_entry_for_12_hours_and_then_nothing() {
        // Cases (b) and (c) of the fixtures README, on the whole fixture and on d1 and d5 each alone.
        let two = vec!["Index repo on desktop".to_string(), "Privacy check".to_string()];
        let m = with_desktop(H, false);
        assert_eq!(desk_rows(&m), (vec![], two.clone()), "(b) no session, two Stopped entries, the twins and the archived one give none");
        assert_eq!(m.counts.stopped, m.stopped.len());
        let mut got: Vec<(&str, &str, u64)> = m.stopped.iter().filter(|w| w.source == "desktop").map(|w| (w.title.as_str(), w.project.as_str(), w.closed)).collect();
        got.sort();
        assert_eq!(got, vec![("Index repo on desktop", "alpha", FIXTURE_ACTIVE), ("Privacy check", "delta-desk", FIXTURE_ACTIVE)]);
        assert!(m.stopped.iter().filter(|w| w.source == "desktop").all(|w| w.ws.is_empty() && !w.dirty && w.machine == "UA"));
        assert_eq!(by_title(&m, "Pod Bay twin"), vec!["podbay"]);
        assert_eq!(by_title(&m, "Remote twin"), vec!["terminal"]);
        // The field the page reads, and no field for a workspace entry.
        let v = serde_json::to_value(&m.stopped).unwrap();
        assert!(v.as_array().unwrap().iter().any(|w| w["source"] == "desktop" && w["title"].is_string() && w["closed"] == FIXTURE_ACTIVE));
        for later in [CLOSED_MS, 13 * H, CLOSED_MS * 20] {
            assert_eq!(desk_rows(&with_desktop(later, false)), (vec![], vec![]), "(c) after {later} ms: nothing, 12 hours is not 'lately'");
        }
        // Each of d1 and d5 alone: (a) live, (b) one hour, (c) 13 hours.
        for (title, project) in [("Index repo on desktop", "alpha"), ("Privacy check", "delta-desk")] {
            let alone = |after: u64, live: bool| {
                let desk: Vec<_> = crate::local::read_desktop(&desktop_fixture()).into_iter().filter(|d| d.title == title).collect();
                let local = if live { vec![live_of(title)] } else { vec![] };
                build(&Raw { desktop: desk, local, now: FIXTURE_ACTIVE + after, ..Raw::default() })
            };
            let a = alone(13 * H, true);
            assert_eq!((a.sessions.len(), a.sessions[0].source, a.sessions[0].title.as_str(), a.sessions[0].project.as_str(), a.stopped.len()), (1, "desktop", title, project, 0));
            let b = alone(H, false);
            assert_eq!((b.sessions.len(), b.stopped.len(), b.stopped[0].source, b.stopped[0].title.as_str(), b.stopped[0].project.as_str()), (0, 1, "desktop", title, project));
            let c = alone(13 * H, false);
            assert_eq!((c.sessions.len(), c.stopped.len()), (0, 0));
        }
        // No clock in the read: nothing can be said about closing, so nothing shows.
        let none = build(&Raw { desktop: crate::local::read_desktop(&desktop_fixture()), ..Raw::default() });
        assert_eq!((none.sessions.len(), none.stopped.len()), (0, 0));
    }

    fn by_title(m: &Model, t: &str) -> Vec<&'static str> {
        m.sessions.iter().filter(|s| s.title == t).map(|s| s.source).collect()
    }

    #[test]
    fn a_twin_is_left_out_only_when_the_other_source_really_shows_it() {
        // The terminal twin has no Remote Control id and the tmux twin has no Pod Bay row: nothing shows those two
        // sessions, so their processes are desktop ones and the desktop files for them show.
        let mut local = crate::local::read(&crate::local::fixture(), FIXTURE_ACTIVE, &|_, _| true);
        for l in local.iter_mut().filter(|l| l.title == "Remote twin") { l.rc_id.clear(); }
        local.extend([live_of("Index repo on desktop"), live_of("Privacy check")]);
        let m = build(&Raw {
            machines: json!([{"machine": "UA", "me": true, "online": true}]),
            local, desktop: crate::local::read_desktop(&desktop_fixture()), now: FIXTURE_ACTIVE + CLOSED_MS + 1, ..Raw::default()
        });
        let mut desk: Vec<&str> = m.sessions.iter().filter(|s| s.source == "desktop").map(|s| s.title.as_str()).collect();
        desk.sort();
        assert_eq!(desk, vec!["Desktop twin of a Pod Bay session", "Desktop twin of a terminal session", "Index repo on desktop", "Privacy check"]);
    }

    #[test]
    fn the_model_of_every_case_of_the_desktop_fixture_holds_no_secret_and_no_cli_id() {
        // Cases (a), (b) and (c) of the fixtures README: sessions and Stopped entries alike. The word `secret` is left
        // out of the whole-fixture model because the local fixture's own sub-agent task holds `~/secret/alpha/tests`
        // (#137); the model of the desktop files alone is checked for it too.
        for (after, live) in [(13 * H, true), (H, false), (13 * H, false)] {
            let full = serde_json::to_string(&with_desktop(after, live)).unwrap();
            let local = if live { vec![live_of("Index repo on desktop"), live_of("Privacy check")] } else { vec![] };
            let alone = serde_json::to_string(&build(&Raw { desktop: crate::local::read_desktop(&desktop_fixture()), local, now: FIXTURE_ACTIVE + after, ..Raw::default() })).unwrap();
            for (name, body) in [("whole fixture", &full), ("desktop files alone", &alone)] {
                let words: &[&str] = if name == "whole fixture" { &["someone"] } else { &["someone", "secret", "worktrees", "Bearer"] };
                for secret in words.iter().chain(&["FIXTURE-SECRET-TOKEN-176", "mcp.example.invalid", "fixture-mcp", "/home/someone", "d1d1d1d1", "d2d2d2d2", "d5d5d5d5",
                    "\"cliSessionId\"", "\"sessionId\"", "local_d2000000", "local_d3000000", "local_d4000000"]) {
                    assert!(!body.contains(secret), "{name}, after {after} live {live}: model holds {secret}");
                }
                assert_eq!(body.contains("local_d1000000-0000-4000-8000-000000000001"), live, "{name}: only a card carries the id that opens it");
            }
        }
    }

    #[test]
    fn the_desktop_model_holds_no_secret_no_full_path_and_only_the_id_that_opens_a_session() {
        let dir = desktop_fixture();
        let snapshot = || -> Vec<(std::path::PathBuf, Vec<u8>)> {
            let mut all = Vec::new();
            let mut stack = vec![dir.clone()];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() { stack.push(p) } else { all.push((p.clone(), std::fs::read(&p).unwrap())) }
                }
            }
            all.sort();
            all
        };
        let before = snapshot();
        // A live process for every file, so every one that is not archived shows as a session.
        let desk = crate::local::read_desktop(&dir);
        let local: Vec<_> = desk.iter().map(|d| crate::local::Local { session_id: d.cli_id.clone(), entrypoint: "claude-desktop".into(), interactive: true, status: "busy".into(), ..Default::default() }).collect();
        let m = build(&Raw { desktop: desk, local, now: FIXTURE_ACTIVE + CLOSED_MS + 1, ..Raw::default() });
        assert_eq!(before, snapshot(), "reading the folder writes nothing and changes no file");
        assert_eq!(m.sessions.iter().filter(|s| s.source == "desktop").count(), 4, "no twin is read here, so d3 and d4 show too");
        let body = serde_json::to_string(&m).unwrap();
        for secret in ["FIXTURE-SECRET-TOKEN-176", "mcp.example.invalid", "fixture-mcp", "/home/someone", "someone", "secret", "worktrees", "Bearer"] {
            assert!(!body.contains(secret), "model holds {secret}");
        }
        // No `cliSessionId`, and no `sessionId` except the one the app needs to open a session: the `sessionId`
        // (`local_<uuid>`) is what the desktop app's own `claude://code/continue?session=` link takes. The CLI ids are
        // never needed to open one, so they never leave.
        for cli in ["d1d1d1d1", "d2d2d2d2", "d5d5d5d5", "33333333-3333", "44444444-4444"] {
            assert!(!body.contains(cli), "model holds the CLI session id {cli}");
        }
        assert!(!body.contains("local_d2000000"), "the archived session's id is not there");
        for field in ["\"cliSessionId\"", "\"sessionId\"", "\"cwd\"", "\"originCwd\"", "\"worktreePath\"", "\"remoteMcpServersConfig\""] {
            assert!(!body.contains(field), "model has the field {field}");
        }
        let ids: Vec<&str> = m.sessions.iter().map(|s| s.desktop_id.as_str()).collect();
        assert!(ids.contains(&"local_d1000000-0000-4000-8000-000000000001") && ids.contains(&"local_d5000000-0000-4000-8000-000000000005"), "{ids:?}");
        assert!(m.sessions.iter().all(|s| s.rc_id.is_empty()), "a desktop session has no Remote Control id");
    }

    #[test]
    fn a_desktop_session_with_a_live_process_takes_its_state_reason_and_sub_agents() {
        let desk = crate::local::read_desktop(&desktop_fixture());
        let idx = desk.iter().find(|d| d.title == "Index repo on desktop").unwrap();
        let live = |status: &str, waiting: &str| crate::local::Local {
            session_id: idx.cli_id.clone(), entrypoint: "claude-desktop".into(), interactive: true, status: status.into(),
            waiting_for: waiting.into(), since: 5, agents: Some(Agents { running: 1, done: 0, list: vec![] }), ..Default::default()
        };
        let one = |l: crate::local::Local| {
            let m = build(&Raw { desktop: desk.clone(), local: vec![l], now: FIXTURE_ACTIVE + 1, ..Raw::default() });
            m.sessions.into_iter().find(|s| s.title == "Index repo on desktop").expect("a live session is a session, not a closed one")
        };
        let w = one(live("waiting", "permission prompt"));
        assert_eq!((w.source, w.state, w.since, w.waiting.as_str()), ("desktop", "needs", 5, "permission prompt"));
        assert_eq!(w.reason.as_ref().map(|r| r.kind), Some("permission"));
        assert_eq!(w.agents.as_ref().map(|a| a.running), Some(1));
        assert_eq!(one(live("busy", "")).state, "busy");
        assert_eq!(one(live("idle", "")).state, "idle");
        assert_eq!(one(live("idle", "input needed")).state, "needs", "a wait text wins over idle");
        assert_eq!(key(&w), "desktop|local_d1000000-0000-4000-8000-000000000001");
    }
    #[test]
    fn a_pod_bay_tmux_session_is_not_a_desktop_row_while_pod_bay_is_unknown() {
        // Issue #176 review F2: the desktop twin of a tmux session must not show while Pod Bay's list is unknown.
        let local = crate::local::read(&crate::local::fixture(), FIXTURE_ACTIVE, &|_, _| true);
        let desk = crate::local::read_desktop(&desktop_fixture());
        let rows = |ls: Value| {
            let m = build(&Raw { ls, machines: json!([{"machine": "UA", "me": true, "online": true}]), local: local.clone(), desktop: desk.clone(), now: FIXTURE_ACTIVE + CLOSED_MS + 1, ..Raw::default() });
            m.sessions.iter().filter(|s| s.title.contains("Pod Bay twin")).map(|s| s.source).collect::<Vec<_>>()
        };
        assert!(rows(json!([{"machine": "UA", "result": "unreachable"}])).is_empty(), "unknown: no row at all, not a Desktop one");
        assert_eq!(rows(json!([{"machine": "UA", "result": [sess(json!({"title": "Pod Bay twin", "name": "claude-pb-bot-02"}))]}])), vec!["podbay"]);
    }
}
