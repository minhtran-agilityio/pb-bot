//! The page model, built from raw `podbay-dev` JSON. Same rules as `podsOf` in plugins/pd-bot/board/board.js.
//! It copies named fields only (an allow list): account, uuid, dir, path and remote_url never leave this module.

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

const WHAT_CHARS: usize = 160;
const NAME_CHARS: usize = 80;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Session {
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
    /// The session's Remote Control link (`remote_url`), kept only when it is a claude.ai/code session link; else empty.
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Stopped {
    pub machine: String,
    pub project: String,
    pub ws: String,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub online: bool,
    /// This machine's Pod Bay holds an out-of-date password for it (`password_current: false`): the two cannot link.
    pub key_stale: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Counts {
    pub needs: usize,
    pub busy: usize,
    pub idle: usize,
    pub stopped: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Model {
    pub machines: Vec<Machine>,
    pub sessions: Vec<Session>,
    pub stopped: Vec<Stopped>,
    pub counts: Counts,
}

/// Raw output of the read-only commands. A value that was never read is `Value::Null`.
#[derive(Debug, Clone, Default)]
pub struct Raw {
    pub ls: Value,
    pub checkouts: Value,
    pub machines: Value,
    pub queue: Value,
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

/// `asks` (a dialog waits for a human) wins over `status`. A session with no status yet has not reached its
/// prompt (seen on a session stopped at a startup dialog), so it needs a human too.
fn state_of(s: &Value) -> &'static str {
    if truthy(&s["asks"]) || s["status"].is_null() {
        "needs"
    } else if s["status"] == "busy" {
        "busy"
    } else {
        "idle"
    }
}

/// A Remote Control link opens the session at claude.ai/code. Anything else is dropped, so a card never opens another site.
pub fn session_url(raw: &str) -> String {
    let ok = raw.strip_prefix("https://claude.ai/code/").map_or(false, |rest| {
        rest.starts_with("session_") && rest.len() > "session_".len() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    if ok { raw.to_string() } else { String::new() }
}

fn rank(state: &str) -> u8 {
    match state {
        "needs" => 0,
        "busy" => 1,
        _ => 2,
    }
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
                url: session_url(&text(&s["remote_url"])),
                prs: s["links"]["prs"].as_array().map(Vec::len).unwrap_or(0),
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
    // Needs-you and busy: the longest wait first. Idle: the most recent first.
    sessions.sort_by(|a, b| {
        rank(a.state)
            .cmp(&rank(b.state))
            .then_with(|| if a.state == "idle" { b.since.cmp(&a.since) } else { a.since.cmp(&b.since) })
            .then_with(|| a.title.cmp(&b.title))
    });

    let stopped: Vec<Stopped> = seen
        .into_iter()
        .map(|((project, ws, machine), dirty)| Stopped { machine, project, ws, dirty })
        .collect();
    let machines = raw
        .machines
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|m| truthy(&m["machine"]))
                .map(|m| Machine { name: text(&m["machine"]), online: truthy(&m["online"]), key_stale: m["password_current"] == Value::Bool(false) })
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
    Model { machines, sessions, stopped, counts }
}

/// The sessions that need a human now and did not at the last read. `before` maps "machine|title" to its last
/// state; an empty map means "first read": nothing is announced then.
pub fn newly_needs<'a>(before: &HashMap<String, &'static str>, now: &'a Model) -> Vec<&'a Session> {
    if before.is_empty() {
        return Vec::new();
    }
    now.sessions
        .iter()
        .filter(|s| s.state == "needs" && before.get(&key(s)).copied() != Some("needs"))
        .collect()
}

pub fn key(s: &Session) -> String {
    format!("{}|{}", s.machine, s.title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        }
    }

    #[test]
    fn only_claude_session_links_are_kept() {
        assert_eq!(session_url("https://claude.ai/code/session_01MjxNHe8J1MMGhFig4bVsHc"), "https://claude.ai/code/session_01MjxNHe8J1MMGhFig4bVsHc");
        for bad in ["", "git@example.com:o/r.git", "https://evil.example/code/session_1", "https://claude.ai/code/", "https://claude.ai/code/session_", "https://claude.ai/code/session_1?x=y", "http://claude.ai/code/session_1", "https://claude.ai/code/session_1/../x"] {
            assert_eq!(session_url(bad), "", "{bad} was kept");
        }
        assert_eq!(build(&raw()).sessions[0].url, "https://claude.ai/code/session_01AbC");
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
        assert_eq!(m.counts, Counts { needs: 3, busy: 1, idle: 2, stopped: 2 });
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
    fn a_workspace_with_no_session_is_stopped_once() {
        let m = build(&raw());
        assert_eq!(m.stopped, vec![
            Stopped { machine: "U0".into(), project: "harness".into(), ws: "src".into(), dirty: false },
            Stopped { machine: "UA".into(), project: "mnemo".into(), ws: "src-07".into(), dirty: true },
        ]);
        assert_eq!(m.machines, vec![
            Machine { name: "UA".into(), online: true, key_stale: false },
            Machine { name: "Old".into(), online: false, key_stale: true },
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
        let odd = Raw { ls: json!("x"), checkouts: json!([{"machine": "A", "result": "err"}]), machines: json!({}), queue: Value::Null };
        assert_eq!(build(&odd), Model::default());
    }

    #[test]
    fn newly_needs_reports_a_change_but_not_the_first_read() {
        let m = build(&raw());
        assert!(newly_needs(&HashMap::new(), &m).is_empty(), "the first read announces nothing");
        let mut before: HashMap<String, &'static str> = m.sessions.iter().map(|s| (key(s), s.state)).collect();
        assert!(newly_needs(&before, &m).is_empty(), "no change, no announcement");
        before.insert("UA|Asks late".into(), "busy");
        before.remove("UA|Starting");
        let titles: Vec<&str> = newly_needs(&before, &m).iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["Starting", "Asks late"], "a changed session and a new one");
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        let long = "ồ".repeat(300);
        let r = Raw { ls: json!([{"machine": "UA", "result": [sess(json!({"description": long}))]}]), ..Raw::default() };
        let what = &build(&r).sessions[0].what;
        assert_eq!(what.chars().count(), WHAT_CHARS);
        assert!(what.ends_with('…'));
    }
}
