//! When to tell the owner that a session needs them: a notification when a session starts to wait, a reminder when
//! it still waits later, a sound with a pause between two, and a flash of the menu bar icon. Do Not Disturb mutes all
//! of them for a while; the board itself keeps updating. Pure logic, so it is tested without a window.

use crate::model::{key, Model, Session};
use std::collections::HashMap;

/// A reminder goes out when a session still needs you this long after it started to wait, and again each time this
/// long passes after the last one.
pub const REMIND_AFTER_MS: u64 = 10 * 60 * 1000;
/// At most this many reminders for one wait.
pub const MAX_REMINDERS: u32 = 3;
/// Two sounds are at least this far apart.
pub const SOUND_GAP_MS: u64 = 10 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The session started to wait.
    New,
    /// The session still waits; the number is which reminder this is (1, 2, …).
    Reminder(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub kind: Kind,
    pub title: String,
    pub body: String,
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

#[derive(Debug, Default)]
pub struct Alerts {
    /// The sessions that wait now, by "machine|title". `None` until the first read, which announces nothing.
    waiting: Option<HashMap<String, Wait>>,
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
        for s in model.sessions.iter().filter(|s| s.state == "needs") {
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
                        });
                    }
                    w
                }
                // A new wait. The first read only learns what already waits.
                None => {
                    if !first && !quiet {
                        out.alerts.push(Alert { kind: Kind::New, title: format!("{} needs you", s.title), body: body(s) });
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

        if !out.alerts.is_empty() {
            out.flash = out.alerts.iter().any(|a| a.kind == Kind::New);
            if self.sound_on && self.last_sound.map_or(true, |t| now.saturating_sub(t) >= SOUND_GAP_MS) {
                out.sound = true;
                self.last_sound = Some(now);
            }
        }
        out
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
        assert_eq!(o.alerts, vec![Alert { kind: Kind::New, title: "New needs you".into(), body: "input needed · pb-bot/src-05 · UA".into() }]);
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
}
