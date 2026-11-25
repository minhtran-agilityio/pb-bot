//! When to tell the owner that a session needs them: a notification when a session starts to wait, a reminder when
//! it still waits later, a sound with a pause between two, and a flash of the menu bar icon. Do Not Disturb mutes all
//! of them for a while; the board itself keeps updating. Pure logic, so it is tested without a window.

use crate::model::{key, order, Model, Session};
use std::collections::HashMap;

/// A reminder goes out when a session still needs you this long after it started to wait, and again each time this
/// long passes after the last one.
pub const REMIND_AFTER_MS: u64 = 10 * 60 * 1000;
/// At most this many reminders for one wait.
pub const MAX_REMINDERS: u32 = 3;
/// Two sounds are at least this far apart.
pub const SOUND_GAP_MS: u64 = 10 * 1000;
/// A session "finished" when it goes idle after working at least this long. A shorter run is a turn of a
/// conversation the owner is in, and is not announced.
pub const FINISH_MIN_RUN_MS: u64 = 2 * 60 * 1000;
/// It must stay idle this long (about two reads) before it counts, so a pause between two steps is not a finish.
pub const FINISH_SETTLE_MS: u64 = 8 * 1000;
/// A finish nobody looked at is forgotten after this long.
pub const FINISH_KEEP_MS: u64 = 12 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The session started to wait.
    New,
    /// The session still waits; the number is which reminder this is (1, 2, …).
    Reminder(u32),
    /// The session finished a long run and is idle. Told once, with no sound and no flash.
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// The session the alert is about (`model::key`), and its Remote Control id when it has one: a click on the
    /// notification opens that session.
    pub session: String,
    pub rc_id: String,
}

/// What to do after one read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub alerts: Vec<Alert>,
    pub sound: bool,
    pub flash: bool,
}

#[derive(Debug, Clone)]
struct Wait {
    machine: String,
    since: u64,
    reminders: u32,
    last_told: u64,
}

/// A session that works now: since when, by this app's clock.
#[derive(Debug, Clone)]
struct Run {
    machine: String,
    since: u64,
}

/// A session that went idle after a long run.
#[derive(Debug, Clone)]
struct Finish {
    machine: String,
    at: u64,
    ran_ms: u64,
    /// It stayed idle for `FINISH_SETTLE_MS`: the board shows it.
    settled: bool,
    told: bool,
}

#[derive(Debug, Default)]
pub struct Alerts {
    /// The sessions that wait now, by "machine|title". `None` until the first read, which announces nothing.
    waiting: Option<HashMap<String, Wait>>,
    running: HashMap<String, Run>,
    finished: HashMap<String, Finish>,
    last_sound: Option<u64>,
    /// Do Not Disturb until this time (ms since the epoch), if set.
    pub dnd_until: Option<u64>,
    pub sound_on: bool,
}

fn place(s: &Session) -> String {
    if s.ws.is_empty() { format!("{} · {}", s.project, s.machine) } else { format!("{}/{} · {}", s.project, s.ws, s.machine) }
}

fn body(s: &Session) -> String {
    let what = if s.what.is_empty() { place(s) } else { s.what.clone() };
    if s.waiting.is_empty() { what } else { format!("{} · {}", s.waiting, what) }
}

/// A Pod Bay session of a machine that was not read this time: the model shows its last known state, which raises
/// nothing (its waits, runs and finishes are kept until the machine is read again).
fn stale(model: &Model, s: &Session) -> bool {
    s.source == "podbay" && model.unread.contains(&s.machine)
}

impl Alerts {
    pub fn new(sound_on: bool) -> Self {
        Alerts { sound_on, ..Alerts::default() }
    }

    /// Whether Do Not Disturb holds at `now`. It ends by itself when its time passes.
    pub fn quiet(&mut self, now: u64) -> bool {
        match self.dnd_until {
            Some(until) if now < until => true,
            Some(_) => {
                self.dnd_until = None;
                false
            }
            None => false,
        }
    }

    /// Feed one read of the model at time `now` (ms since the epoch).
    pub fn on_read(&mut self, model: &Model, now: u64) -> Outcome {
        let quiet = self.quiet(now);
        let first = self.waiting.is_none();
        let before = self.waiting.take().unwrap_or_default();
        let mut waiting = HashMap::new();
        let mut out = Outcome::default();
        self.track_finishes(model, now, first, quiet, &mut out);
        for s in model.sessions.iter().filter(|s| s.state == "needs" && !stale(model, s)) {
            let k = key(s);
            let wait = match before.get(&k) {
                // A wait that goes on: remind when its time has come.
                Some(w) => {
                    let mut w = w.clone();
                    // A muted reminder is not spent: it goes out once Do Not Disturb ends.
                    if !quiet && w.reminders < MAX_REMINDERS && now.saturating_sub(w.last_told) >= REMIND_AFTER_MS {
                        w.reminders += 1;
                        w.last_told = now;
                        let minutes = now.saturating_sub(w.since) / 60_000;
                        out.alerts.push(Alert {
                            kind: Kind::Reminder(w.reminders),
                            title: format!("{} still needs you ({minutes} min)", s.title),
                            body: body(s),
                            session: k.clone(),
                            rc_id: s.rc_id.clone(),
                        });
                    }
                    w
                }
                // A new wait. The first read only learns what already waits.
                None => {
                    if !first && !quiet {
                        out.alerts.push(Alert { kind: Kind::New, title: format!("{} needs you", s.title), body: body(s), session: k.clone(), rc_id: s.rc_id.clone() });
                    }
                    Wait { machine: s.machine.clone(), since: now, reminders: 0, last_told: now }
                }
            };
            waiting.insert(k, wait);
        }
        // A machine that could not be read this time keeps its waits, so its return is not announced as new.
        for (k, w) in before {
            if model.unread.contains(&w.machine) { waiting.entry(k).or_insert(w); }
        }
        self.waiting = Some(waiting);

        // A finish is a quiet notice: only a wait or its reminder makes a sound.
        if out.alerts.iter().any(|a| a.kind != Kind::Finished) {
            out.flash = out.alerts.iter().any(|a| a.kind == Kind::New);
            if self.sound_on && self.last_sound.map_or(true, |t| now.saturating_sub(t) >= SOUND_GAP_MS) {
                out.sound = true;
                self.last_sound = Some(now);
            }
        }
        out
    }

    /// Follow each session from working to idle. A session that worked for `FINISH_MIN_RUN_MS` and then stays idle
    /// for `FINISH_SETTLE_MS` has finished: the board marks it until the owner looks at it, and one quiet
    /// notification goes out (after Do Not Disturb, when that holds).
    fn track_finishes(&mut self, model: &Model, now: u64, first: bool, quiet: bool, out: &mut Outcome) {
        let mut seen: Vec<String> = Vec::new();
        for s in model.sessions.iter().filter(|s| !stale(model, s)) {
            let k = key(s);
            match s.state {
                "busy" => {
                    self.finished.remove(&k);
                    // On the first read the run started before this app looked: take Pod Bay's time for it.
                    let since = if first && s.since > 0 && (s.since as u64) <= now { s.since as u64 } else { now };
                    self.running.entry(k.clone()).or_insert(Run { machine: s.machine.clone(), since });
                }
                "idle" => {
                    if let Some(run) = self.running.remove(&k) {
                        let ran_ms = now.saturating_sub(run.since);
                        if ran_ms >= FINISH_MIN_RUN_MS {
                            self.finished.insert(k.clone(), Finish { machine: run.machine, at: now, ran_ms, settled: false, told: false });
                        }
                    }
                    if let Some(f) = self.finished.get_mut(&k) {
                        if now.saturating_sub(f.at) >= FINISH_SETTLE_MS { f.settled = true; }
                        if f.settled && !f.told && !quiet {
                            f.told = true;
                            out.alerts.push(Alert {
                                kind: Kind::Finished,
                                title: format!("{} finished", s.title),
                                body: format!("Worked {} min · {}", f.ran_ms / 60_000, if s.what.is_empty() { place(s) } else { s.what.clone() }),
                                session: k.clone(),
                                rc_id: s.rc_id.clone(),
                            });
                        }
                    }
                }
                // It asks instead: the wait is announced, and the run is over without a finish.
                _ => {
                    self.running.remove(&k);
                    self.finished.remove(&k);
                }
            }
            seen.push(k);
        }
        // A session that is gone is forgotten, unless its machine only could not be read this time.
        self.running.retain(|k, r| seen.contains(k) || model.unread.contains(&r.machine));
        self.finished.retain(|k, f| (seen.contains(k) || model.unread.contains(&f.machine)) && now.saturating_sub(f.at) < FINISH_KEEP_MS);
    }

    /// Write the finishes into the model: `finished` on each session, the count, and the finished ones first.
    pub fn mark(&self, model: &mut Model) {
        let mut n = 0;
        for s in model.sessions.iter_mut() {
            s.finished = match self.finished.get(&key(s)) {
                Some(f) if f.settled && s.state == "idle" => { n += 1; f.at }
                _ => 0,
            };
        }
        model.counts.finished = n;
        order(&mut model.sessions);
    }

    /// The owner looked at this session (opened it, peeked at it, or clicked its notification): the finish is done.
    /// True when there was one.
    pub fn looked(&mut self, session: &str) -> bool {
        self.finished.remove(session).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Counts;

    fn s(title: &str, state: &'static str) -> Session {
        Session {
            machine: "UA".into(), project: "pb-bot".into(), ws: "src-05".into(), title: title.into(), state,
            since: 0, what: String::new(), branch: String::new(), prs: 0, queued: 0, rc_id: String::new(), name: String::new(),
            links: vec![], finished: 0, source: "podbay", reason: None, agents: None,
            waiting: if state == "needs" { "input needed".into() } else { String::new() },
        }
    }
    fn m(list: Vec<Session>) -> Model {
        Model { machines: vec![], sessions: list, stopped: vec![], counts: Counts::default(), unread: vec![] }
    }
    const MIN: u64 = 60_000;

    #[test]
    fn the_first_read_is_silent_then_a_new_wait_is_told_once() {
        let mut a = Alerts::new(true);
        assert_eq!(a.on_read(&m(vec![s("Old", "needs")]), 0), Outcome::default(), "what already waits at start is not announced");
        let o = a.on_read(&m(vec![s("Old", "needs"), s("New", "needs")]), 5_000);
        assert_eq!(o.alerts, vec![Alert {
            kind: Kind::New, title: "New needs you".into(), body: "input needed · pb-bot/src-05 · UA".into(),
            session: "UA|pb-bot/src-05|New".into(), rc_id: String::new(),
        }]);
        assert!(o.sound && o.flash);
        assert!(a.on_read(&m(vec![s("Old", "needs"), s("New", "needs")]), 8_000).alerts.is_empty(), "no repeat on the next read");
    }

    #[test]
    fn reminders_come_every_ten_minutes_at_most_three_times() {
        let mut a = Alerts::new(false);
        a.on_read(&m(vec![]), 0);
        a.on_read(&m(vec![s("W", "needs")]), MIN);
        let mut told = vec![];
        for minute in 2..60 {
            for al in a.on_read(&m(vec![s("W", "needs")]), minute * MIN).alerts { told.push((minute, al.kind, al.title)); }
        }
        assert_eq!(told, vec![
            (11, Kind::Reminder(1), "W still needs you (10 min)".into()),
            (21, Kind::Reminder(2), "W still needs you (20 min)".into()),
            (31, Kind::Reminder(3), "W still needs you (30 min)".into()),
        ]);
    }

    #[test]
    fn a_reminder_does_not_flash_and_a_wait_that_ends_starts_over() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        a.on_read(&m(vec![s("W", "needs")]), 0);
        let o = a.on_read(&m(vec![s("W", "needs")]), REMIND_AFTER_MS);
        assert_eq!(o.alerts.len(), 1);
        assert!(!o.flash && o.sound);
        a.on_read(&m(vec![s("W", "idle")]), REMIND_AFTER_MS + MIN);
        let o = a.on_read(&m(vec![s("W", "needs")]), REMIND_AFTER_MS + 2 * MIN);
        assert_eq!(o.alerts[0].kind, Kind::New, "waiting again after an answer is a new wait");
    }

    #[test]
    fn sounds_keep_a_gap_and_can_be_off() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        assert!(a.on_read(&m(vec![s("A", "needs")]), 1_000).sound);
        let o = a.on_read(&m(vec![s("A", "needs"), s("B", "needs")]), 5_000);
        assert!(!o.alerts.is_empty() && !o.sound, "a second sound within 10 s is skipped");
        assert!(a.on_read(&m(vec![s("A", "needs"), s("B", "needs"), s("C", "needs")]), 12_000).sound);
        a.sound_on = false;
        assert!(!a.on_read(&m(vec![s("A", "needs"), s("B", "needs"), s("C", "needs"), s("D", "needs")]), 60_000).sound);
    }

    #[test]
    fn a_machine_that_drops_out_for_a_read_is_not_announced_again() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        assert_eq!(a.on_read(&m(vec![s("X", "needs")]), MIN).alerts.len(), 1);
        let mut gone = m(vec![]); gone.unread = vec!["UA".into()];
        assert!(a.on_read(&gone, 2 * MIN).alerts.is_empty());
        assert!(a.on_read(&m(vec![s("X", "needs")]), 3 * MIN).alerts.is_empty(), "back after a blip: no new alert");
        let o = a.on_read(&m(vec![s("X", "needs")]), 11 * MIN);
        assert_eq!(o.alerts[0].kind, Kind::Reminder(1), "the reminder clock kept running");
    }

    #[test]
    fn sessions_are_told_apart_by_link_or_workspace_and_survive_a_rename() {
        let mut a = Alerts::new(false);
        a.on_read(&m(vec![]), 0);
        let mut one = s("Untitled", "needs"); one.ws = "src-01".into();
        let mut two = s("Untitled", "needs"); two.ws = "src-02".into();
        assert_eq!(a.on_read(&m(vec![one.clone()]), MIN).alerts.len(), 1);
        assert_eq!(a.on_read(&m(vec![one.clone(), two]), 2 * MIN).alerts.len(), 1, "a second same-named session is its own wait");
        let mut linked = s("Draft", "needs"); linked.rc_id = "session_01X".into();
        assert_eq!(a.on_read(&m(vec![linked.clone()]), 3 * MIN).alerts.len(), 1);
        linked.title = "Fix the login bug".into();
        assert!(a.on_read(&m(vec![linked]), 4 * MIN).alerts.is_empty(), "a renamed session keeps its wait");
    }

    fn finished(a: &Alerts, list: Vec<Session>) -> Vec<(String, u64)> {
        let mut model = m(list);
        a.mark(&mut model);
        model.sessions.iter().filter(|s| s.finished > 0).map(|s| (s.title.clone(), s.finished)).collect()
    }

    #[test]
    fn a_long_run_that_goes_idle_is_told_once_without_sound_and_marked_until_looked_at() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        a.on_read(&m(vec![s("Job", "busy")]), MIN);
        assert!(a.on_read(&m(vec![s("Job", "idle")]), 6 * MIN).alerts.is_empty(), "not yet: it may only pause between two steps");
        assert!(finished(&a, vec![s("Job", "idle")]).is_empty());
        let o = a.on_read(&m(vec![s("Job", "idle")]), 6 * MIN + FINISH_SETTLE_MS);
        assert_eq!(o.alerts, vec![Alert {
            kind: Kind::Finished, title: "Job finished".into(), body: "Worked 5 min · pb-bot/src-05 · UA".into(),
            session: "UA|pb-bot/src-05|Job".into(), rc_id: String::new(),
        }]);
        assert!(!o.sound && !o.flash, "a finish is quiet");
        assert!(a.on_read(&m(vec![s("Job", "idle")]), 20 * MIN).alerts.is_empty(), "told once, no reminder");
        assert_eq!(finished(&a, vec![s("Job", "idle")]), vec![("Job".to_string(), 6 * MIN)]);
        let mut model = m(vec![s("Job", "idle")]);
        a.mark(&mut model);
        assert_eq!(model.counts.finished, 1);
        assert!(a.looked("UA|pb-bot/src-05|Job") && !a.looked("UA|pb-bot/src-05|Job"));
        assert!(finished(&a, vec![s("Job", "idle")]).is_empty(), "looked at: the mark is gone");
    }

    #[test]
    fn a_short_run_a_pause_and_a_question_are_not_a_finish() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        // A short turn of a conversation.
        a.on_read(&m(vec![s("Chat", "busy")]), MIN);
        a.on_read(&m(vec![s("Chat", "idle")]), MIN + 30_000);
        assert!(a.on_read(&m(vec![s("Chat", "idle")]), 2 * MIN).alerts.is_empty());
        // A long run that pauses for one read and works on.
        a.on_read(&m(vec![s("Steps", "busy")]), 10 * MIN);
        a.on_read(&m(vec![s("Steps", "idle")]), 15 * MIN);
        a.on_read(&m(vec![s("Steps", "busy")]), 15 * MIN + 4_000);
        assert!(a.on_read(&m(vec![s("Steps", "busy")]), 15 * MIN + FINISH_SETTLE_MS).alerts.is_empty());
        assert!(finished(&a, vec![s("Steps", "busy")]).is_empty());
        // A long run that ends with a question: the wait is told, not a finish.
        a.on_read(&m(vec![s("Ask", "busy")]), 20 * MIN);
        let o = a.on_read(&m(vec![s("Ask", "needs")]), 30 * MIN);
        assert_eq!(o.alerts.iter().map(|x| x.kind.clone()).collect::<Vec<_>>(), vec![Kind::New]);
        assert!(a.on_read(&m(vec![s("Ask", "idle")]), 31 * MIN).alerts.is_empty(), "answered and idle: nothing more");
    }

    #[test]
    fn a_run_that_began_before_the_app_counts_from_pod_bays_time() {
        let mut a = Alerts::new(false);
        let mut old = s("Old", "busy"); old.since = (5 * MIN) as i64;
        a.on_read(&m(vec![old]), 60 * MIN);
        a.on_read(&m(vec![s("Old", "idle")]), 61 * MIN);
        let o = a.on_read(&m(vec![s("Old", "idle")]), 62 * MIN);
        assert_eq!(o.alerts[0].body, "Worked 56 min · pb-bot/src-05 · UA");
    }

    #[test]
    fn a_finish_waits_for_do_not_disturb_to_end_and_is_forgotten_with_its_session() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        a.on_read(&m(vec![s("Job", "busy")]), MIN);
        a.dnd_until = Some(30 * MIN);
        a.on_read(&m(vec![s("Job", "idle")]), 10 * MIN);
        assert_eq!(a.on_read(&m(vec![s("Job", "idle")]), 11 * MIN), Outcome::default(), "muted");
        assert_eq!(finished(&a, vec![s("Job", "idle")]).len(), 1, "the board still marks it");
        assert_eq!(a.on_read(&m(vec![s("Job", "idle")]), 31 * MIN).alerts[0].kind, Kind::Finished);
        let mut gone = m(vec![]); gone.unread = vec!["UA".into()];
        a.on_read(&gone, 32 * MIN);
        assert_eq!(finished(&a, vec![s("Job", "idle")]).len(), 1, "its machine could not be read: kept");
        a.on_read(&m(vec![]), 33 * MIN);
        assert!(finished(&a, vec![s("Job", "idle")]).is_empty(), "the session is gone");
        a.on_read(&m(vec![s("Late", "busy")]), 40 * MIN);
        a.on_read(&m(vec![s("Late", "idle")]), 50 * MIN);
        a.on_read(&m(vec![s("Late", "idle")]), 51 * MIN);
        a.on_read(&m(vec![s("Late", "idle")]), 50 * MIN + FINISH_KEEP_MS);
        assert!(finished(&a, vec![s("Late", "idle")]).is_empty(), "nobody looked for twelve hours");
    }

    #[test]
    fn do_not_disturb_mutes_until_it_ends_and_keeps_tracking() {
        let mut a = Alerts::new(true);
        a.on_read(&m(vec![]), 0);
        a.dnd_until = Some(30 * MIN);
        assert_eq!(a.on_read(&m(vec![s("Q", "needs")]), MIN), Outcome::default(), "muted while it holds");
        assert_eq!(a.on_read(&m(vec![s("Q", "needs")]), 11 * MIN), Outcome::default(), "no reminder either");
        assert!(a.quiet(29 * MIN));
        assert!(!a.quiet(30 * MIN) && a.dnd_until.is_none(), "it ends by itself");
        let o = a.on_read(&m(vec![s("Q", "needs"), s("R", "needs")]), 31 * MIN);
        let titles: Vec<&str> = o.alerts.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(titles, vec!["Q still needs you (30 min)", "R needs you"],
            "the muted start of Q is not announced late, but its reminder goes out once the quiet time ends");
    }

    #[test]
    fn a_local_session_takes_part_in_the_alert_rules() {
        use crate::local::Local;
        use crate::model::{build, Raw};
        let me = serde_json::json!([{"machine": "UA", "me": true, "online": true}]);
        let read = |status: &str, waiting: &str| build(&Raw {
            machines: me.clone(),
            local: vec![Local { rc_id: "session_01Term".into(), entrypoint: "cli".into(), interactive: true, status: status.into(),
                                waiting_for: waiting.into(), title: "Terminal job".into(), project: "alpha".into(), since: 1, ..Local::default() }],
            ..Raw::default()
        });
        let mut a = Alerts::new(true);
        assert_eq!(a.on_read(&read("busy", ""), 0), Outcome::default());
        // A new wait: one alert, with sound and flash, opening the session by its Remote Control id.
        let o = a.on_read(&read("waiting", "permission prompt"), 5_000);
        assert_eq!(o.alerts.len(), 1);
        assert_eq!((o.alerts[0].kind.clone(), o.alerts[0].rc_id.as_str(), o.sound, o.flash), (Kind::New, "session_01Term", true, true));
        assert!(a.on_read(&read("waiting", "permission prompt"), 10_000).alerts.is_empty(), "told once");
        // A 2 minute run, then idle: finished.
        a.on_read(&read("busy", ""), 20_000);
        a.on_read(&read("idle", ""), 20_000 + FINISH_MIN_RUN_MS);
        let o = a.on_read(&read("idle", ""), 20_000 + FINISH_MIN_RUN_MS + FINISH_SETTLE_MS);
        assert_eq!(o.alerts.iter().map(|x| x.kind.clone()).collect::<Vec<_>>(), vec![Kind::Finished]);
        let mut m = read("idle", "");
        a.mark(&mut m);
        assert!(m.sessions[0].finished > 0 && m.counts.finished == 1);
    }
}
