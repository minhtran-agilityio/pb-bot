// Pod Board: a small desktop window with every Pod Bay session by state (needs you, busy, idle).
// It only observes. It polls the read-only commands `podbay-dev ls --all`, `checkouts --all`, `machines` and
// `queue ls --all`. It runs `podbay-dev detail` only when the owner opens a session's peek, for that one session,
// and keeps only its last prompt and reply (never the screen). It never sends anything anywhere. A click can open
// a session in the Claude app, or one of its links (a pull request, an issue, an artifact) in the browser.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alerts;
mod model;

use alerts::Alerts;
use model::{build, Model, Raw};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;

const POLL_PAUSE: Duration = Duration::from_secs(3); // one `ls --all` takes about 1.3 s
const SLOW_EVERY: u32 = 10; // checkouts, machines and the queue change slowly
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// `detail` on another machine took 10.5 s on 2026-11-21, so the peek waits longer than the poll does.
const PEEK_TIMEOUT: Duration = Duration::from_secs(25);
/// The popover under the menu bar icon, in logical pixels (keep in step with `popover` in tauri.conf.json).
const POPOVER_W: f64 = 340.0;
const MAX_OUTPUT: u64 = 16 << 20;

#[derive(Debug, Clone, Default, Serialize)]
struct Snapshot {
    ok: bool,
    error: Option<String>,
    read_at: Option<u64>,
    /// Do Not Disturb until this time (ms since the epoch), when on.
    dnd_until: Option<u64>,
    #[serde(flatten)]
    model: Model,
}

#[derive(Default)]
struct Shared(Mutex<Snapshot>);

/// Wakes the poll loop before its pause ends, for the Refresh button.
struct Wake(Mutex<Sender<()>>);

/// When to notify, remind, sound and flash (see alerts.rs).
struct AlertState(Mutex<Alerts>);

/// The notifications that were shown, by their id: the session each is about (`model::key`) and its Remote Control
/// id. A click on a notification looks its session up here.
#[derive(Default)]
struct Told(Mutex<HashMap<i32, (String, String)>>);
static NEXT_NOTE: AtomicI32 = AtomicI32::new(1);
const TOLD_MAX: usize = 200;

/// The Do Not Disturb submenu, so its title can say whether it is on.
struct DndMenu(Mutex<Option<Submenu<Wry>>>);

/// The settings that survive a restart: the sound switch and the end of Do Not Disturb.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    sound: Option<bool>,
    dnd_until: Option<u64>,
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

fn load_settings(app: &AppHandle) -> Settings {
    settings_path(app).and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// One save at a time, written to a temporary file and renamed over the old one, so a crash or two saves at once
/// never leave a half-written file.
static SAVE: Mutex<()> = Mutex::new(());

fn save_settings(app: &AppHandle) {
    let Some(path) = settings_path(app) else { return };
    let _one = SAVE.lock();
    let (sound, dnd_until) = match app.state::<AlertState>().0.lock() { Ok(a) => (a.sound_on, a.dnd_until), Err(_) => return };
    if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
    let Ok(text) = serde_json::to_string(&Settings { sound: Some(sound), dnd_until }) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() { let _ = std::fs::rename(&tmp, &path); }
}

/// Turn Do Not Disturb on for `minutes`, or off with 0. Used by the menu and by the window's "End" button.
fn apply_dnd(app: &AppHandle, minutes: u64) {
    if let Ok(mut a) = app.state::<AlertState>().0.lock() {
        a.dnd_until = if minutes == 0 { None } else { Some(now_ms() + minutes * 60_000) };
    }
    save_settings(app);
    if let Ok(tx) = app.state::<Wake>().0.lock() { let _ = tx.send(()); }
}

#[tauri::command]
fn set_dnd(app: AppHandle, minutes: u64) {
    apply_dnd(&app, minutes.min(24 * 60));
}

#[tauri::command]
fn refresh(wake: tauri::State<'_, Wake>) {
    if let Ok(tx) = wake.0.lock() {
        let _ = tx.send(());
    }
}

/// Open a session in the Claude desktop app with `claude://code/<id>`, the link Pod Bay's "Open in Claude" uses.
/// Without the desktop app (Linux) it opens `https://claude.ai/code/<id>` in the browser instead.
fn open_claude(id: &str) -> Result<(), String> {
    if id.is_empty() || model::session_id(&format!("https://claude.ai/code/{id}")) != id {
        return Err("not a Claude session id".into());
    }
    #[cfg(target_os = "macos")]
    let (opener, url) = ("open", format!("claude://code/{id}"));
    #[cfg(not(target_os = "macos"))]
    let (opener, url) = ("xdg-open", format!("https://claude.ai/code/{id}"));
    spawn_reaped(Command::new(opener).arg(&url)).map_err(|_| "could not open the session".to_string())
}

/// The owner looked at a session: its "finished" mark goes, at once on the page and in the menu bar.
fn looked(app: &AppHandle, session: &str) {
    let had = app.state::<AlertState>().0.lock().map(|mut a| a.looked(session)).unwrap_or(false);
    if !had { return; }
    let snap = {
        let state = app.state::<Shared>();
        let mut held = match state.0.lock() { Ok(g) => g, Err(p) => p.into_inner() };
        if let Ok(a) = app.state::<AlertState>().0.lock() { a.mark(&mut held.model); }
        held.clone()
    };
    publish(app, &snap);
}

/// The key (`model::key`) of the session the snapshot holds for `pick`, if any.
fn session_key(app: &AppHandle, pick: impl Fn(&model::Session) -> bool) -> Option<String> {
    let state = app.state::<Shared>();
    let held = state.0.lock().ok()?;
    held.model.sessions.iter().find(|s| pick(s)).map(model::key)
}

#[tauri::command]
fn open_session(app: AppHandle, id: String) -> Result<(), String> {
    open_claude(&id)?;
    // A session with a Remote Control id has that id as its key.
    looked(&app, &id);
    Ok(())
}

/// Open one of a session's links (a pull request, an issue, an artifact) in the browser. Only an address the
/// snapshot holds right now passes, so the page cannot make the app open anything else.
#[tauri::command]
fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    let known = !url.is_empty() && model::safe_url(&url) == url && {
        let state = app.state::<Shared>();
        let held = state.0.lock().map_err(|_| "no data".to_string())?;
        held.model.sessions.iter().any(|s| s.links.iter().any(|l| l.url == url))
    };
    if !known { return Err("not a link of a session".into()); }
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    spawn_reaped(Command::new(opener).arg(&url)).map_err(|_| "could not open the link".to_string())
}

/// What the peek shows of one session: its last prompt and its last reply, both plain text and cut short.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
struct Peek {
    prompt: String,
    reply: String,
    /// When the reply was written (ms since the epoch), when Pod Bay says.
    at: Option<u64>,
    /// False when that machine does not share its sessions' text.
    shared: bool,
}

const PEEK_PROMPT_CHARS: usize = 1200;
const PEEK_REPLY_CHARS: usize = 4000;

/// Keep the tail of a long text: the end of a reply holds the question.
fn tail(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n { return s.to_string(); }
    let mut out = String::from("…");
    out.extend(s.chars().skip(count - (n - 1)));
    out
}

/// Pod Bay sends a long reply in two cut pieces: `activity[].full` is its head ("…" at the end) and `reply` is its
/// tail ("…" at the start). Join them where they overlap, else with a "…" line between; the tail holds the question.
fn join_reply(head: &str, tail_part: &str) -> String {
    let (head, tail_part) = (head.trim(), tail_part.trim());
    let Some(h) = head.strip_suffix('…') else { return head.to_string() };
    let Some(t) = tail_part.strip_prefix('…') else { return if tail_part.is_empty() { head.to_string() } else { tail_part.to_string() } };
    // The head is the start of the reply and the tail its end, so where they overlap the end of the head equals the
    // start of the tail. Take the longest such overlap of at least 12 characters.
    let cuts: Vec<usize> = t.char_indices().map(|(i, _)| i).chain(std::iter::once(t.len())).collect();
    for &k in cuts.iter().rev() {
        if t[..k].chars().count() < 12 { break; }
        if h.ends_with(&t[..k]) {
            return format!("{}{}", h, &t[k..]);
        }
    }
    format!("{}\n\n…\n\n{}", h.trim_end(), t.trim_start())
}

/// Pick the prompt and reply out of `podbay-dev detail` JSON: the newest reply in `activity`, joined with `reply`.
/// The screen (`peek`) is dropped: it can show the account and anything else on the terminal.
fn peek_of(v: &Value) -> Peek {
    let newest = v["activity"].as_array().and_then(|a| {
        a.iter().filter(|e| e["kind"] == "reply").max_by_key(|e| e["at"].as_u64().unwrap_or(0))
    });
    let tail_part = v["reply"].as_str().unwrap_or("");
    let reply = match newest.and_then(|e| e["full"].as_str()) {
        Some(head) => join_reply(head, tail_part),
        None => tail_part.trim().to_string(),
    };
    Peek {
        prompt: tail(v["prompt"].as_str().unwrap_or("").trim(), PEEK_PROMPT_CHARS),
        reply: tail(&reply, PEEK_REPLY_CHARS),
        at: newest.and_then(|e| e["at"].as_u64()),
        shared: v["shared"].as_bool().unwrap_or(true),
    }
}

/// The last prompt and reply of one session, read on demand with `podbay-dev --on <machine> detail <name>`.
/// It runs off the main thread: another machine can take ten seconds or more to answer.
#[tauri::command]
async fn peek(app: AppHandle, machine: String, name: String) -> Result<Peek, String> {
    let (machine, name) = (model::safe_name(&machine), model::safe_name(&name));
    if machine.is_empty() || name.is_empty() {
        return Err("not a Pod Bay session".into());
    }
    // Reading its last reply is looking at it.
    if let Some(k) = session_key(&app, |s| s.machine == machine && s.name == name) { looked(&app, &k); }
    tauri::async_runtime::spawn_blocking(move || {
        read_for(&find_podbay(), &["--on", &machine, "detail", &name], PEEK_TIMEOUT).map(|v| peek_of(&v))
    })
    .await
    .map_err(|_| "podbay-dev detail failed".to_string())?
}

#[tauri::command]
fn show_board(app: AppHandle) {
    hide_popover(&app);
    show_main(&app);
}

#[tauri::command]
fn close_popover(app: AppHandle) {
    hide_popover(&app);
}

#[tauri::command]
fn snapshot(state: tauri::State<'_, Shared>) -> Snapshot {
    state.0.lock().map(|s| s.clone()).unwrap_or_default()
}

/// A desktop app starts with a short PATH, so also look where Pod Bay installs its CLI.
fn find_podbay() -> PathBuf {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local").join("bin"));
    }
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.into_iter().map(|d| d.join("podbay-dev")).find(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("podbay-dev"))
}

/// Run one read-only Pod Bay command and parse its JSON. The error text names the subcommand, never stderr.
fn read(cmd: &PathBuf, args: &[&str]) -> Result<Value, String> {
    read_for(cmd, args, COMMAND_TIMEOUT)
}

/// `read` with its own time limit. The subcommand in errors is the first argument that is not a flag or its value.
fn read_for(cmd: &PathBuf, args: &[&str], timeout: Duration) -> Result<Value, String> {
    let sub = if args.first() == Some(&"--on") { args.get(2).copied().unwrap_or("") } else { args[0] };
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { "podbay-dev not found".to_string() } else { format!("podbay-dev {} did not start", sub) })?;
    let mut out = child.stdout.take().ok_or_else(|| format!("podbay-dev {} gave no output", sub))?;
    // The reader hands its text over a channel, so a helper that keeps the pipe open after the command exits
    // cannot hang the poll loop. The output is capped (Pod Bay's JSON is tens of KB).
    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = (&mut out).take(MAX_OUTPUT).read_to_string(&mut text);
        let _ = tx.send(text);
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("podbay-dev {} timed out", sub));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => return Err(format!("podbay-dev {} failed", sub)),
        }
    };
    let left = timeout.saturating_sub(started.elapsed()).max(Duration::from_secs(1));
    let text = rx.recv_timeout(left).map_err(|_| format!("podbay-dev {} kept its output open", sub))?;
    if !status.success() {
        return Err(format!("podbay-dev {} failed", sub));
    }
    serde_json::from_str(&text).map_err(|_| format!("podbay-dev {} did not return JSON", sub))
}

/// Start a short-lived helper (`open`, `afplay`) and reap it in the background, so it leaves no zombie.
fn spawn_reaped(cmd: &mut Command) -> std::io::Result<()> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The menu bar icon for the most urgent state, and the count beside it (empty when nothing counts).
/// Icons: Lucide (ISC), the same shapes as the window, drawn black so macOS can tint them as template images.
fn tray_look(snap: &Snapshot) -> (&'static [u8], String) {
    let c = snap.model.counts;
    // Nobody waits, but something finished: say so beside the icon ("✓2", or "3 ✓2" while others work).
    let done = if c.finished > 0 { format!("✓{}", c.finished) } else { String::new() };
    if !snap.ok {
        (include_bytes!("../icons/tray-triangle-alert.png"), String::new())
    } else if c.needs > 0 {
        (include_bytes!("../icons/tray-hand.png"), c.needs.to_string())
    } else if c.busy > 0 {
        (include_bytes!("../icons/tray-loader.png"), format!("{} {done}", c.busy).trim_end().to_string())
    } else if c.idle > 0 {
        (include_bytes!("../icons/tray-coffee.png"), done)
    } else {
        (include_bytes!("../icons/tray-moon-star.png"), String::new())
    }
}

fn publish(app: &AppHandle, snap: &Snapshot) {
    let _ = app.emit("pods", snap);
    if let Some(tray) = app.tray_by_id("pod-board") {
        let (icon, title) = tray_look(snap);
        if let Ok(image) = tauri::image::Image::from_bytes(icon) {
            let _ = tray.set_icon(Some(image));
            let _ = tray.set_icon_as_template(true);
        }
        let _ = tray.set_title(if title.is_empty() { None } else { Some(title) });
        let c = snap.model.counts;
        let quiet = if snap.dnd_until.is_some() { " · Do not disturb" } else { "" };
        let done = if c.finished > 0 { format!(" · {} finished", c.finished) } else { String::new() };
        let _ = tray.set_tooltip(Some(format!("{} need you · {} working · {} idle{done}{quiet}", c.needs, c.busy, c.idle)));
    }
    if let Some(window) = app.get_webview_window("main") {
        let needs = snap.model.counts.needs as i64;
        let _ = window.set_badge_count(if needs > 0 { Some(needs) } else { None });
    }
}

/// Show one notification and remember which session it is about, for a click on it (see `on_note_click`).
fn notify(app: &AppHandle, alert: &alerts::Alert) {
    let id = NEXT_NOTE.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut told) = app.state::<Told>().0.lock() {
        if told.len() >= TOLD_MAX { told.clear(); }
        told.insert(id, (alert.session.clone(), alert.rc_id.clone()));
    }
    let _ = app.notification().builder().id(id).title(&alert.title).body(&alert.body).show();
}

/// A click on a notification opens its session in the Claude app. A session with no Remote Control id, or a
/// notification this run did not show, opens the board instead.
fn on_note_click(app: &AppHandle, id: Option<i32>) {
    let about = id.and_then(|id| app.state::<Told>().0.lock().ok().and_then(|mut told| told.remove(&id)));
    match about {
        Some((session, rc_id)) => {
            if rc_id.is_empty() || open_claude(&rc_id).is_err() { show_main(app); }
            looked(app, &session);
        }
        None => show_main(app),
    }
}

/// A short system sound. macOS only; elsewhere the notification is the only signal.
fn play_sound() {
    #[cfg(target_os = "macos")]
    let _ = spawn_reaped(Command::new("afplay").arg("/System/Library/Sounds/Glass.aiff"));
}

/// Blink the menu bar icon a few times, then put back the icon of the current state.
fn flash_tray(app: AppHandle) {
    std::thread::spawn(move || {
        let hand = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-hand.png"));
        let blank = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-blank.png"));
        let (Ok(hand), Ok(blank)) = (hand, blank) else { return };
        for i in 0..8 {
            if let Some(tray) = app.tray_by_id("pod-board") {
                let _ = tray.set_icon(Some(if i % 2 == 0 { blank.clone() } else { hand.clone() }));
                let _ = tray.set_icon_as_template(true);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        let snap = app.state::<Shared>().0.lock().map(|s| s.clone()).unwrap_or_default();
        publish(&app, &snap);
    });
}

fn poll_loop(app: AppHandle, wake: Receiver<()>) {
    let cmd = find_podbay();
    let mut raw = Raw::default();
    let mut n: u32 = 0;
    loop {
        let mut error = None;
        match read(&cmd, &["ls", "--all"]) {
            Ok(v) => {
                raw.ls = v;
                if n % SLOW_EVERY == 0 {
                    // A failed slow read keeps its last value.
                    if let Ok(v) = read(&cmd, &["checkouts", "--all"]) { raw.checkouts = v; }
                    if let Ok(v) = read(&cmd, &["machines"]) { raw.machines = v; }
                    if let Ok(v) = read(&cmd, &["queue", "ls", "--all"]) { raw.queue = v; }
                }
                n = n.wrapping_add(1);
            }
            Err(e) => error = Some(e),
        }

        let mut outcome = alerts::Outcome::default();
        let snap = {
            let state = app.state::<Shared>();
            let mut held = match state.0.lock() { Ok(g) => g, Err(p) => p.into_inner() };
            match error {
                // A failed read keeps the last sessions and says why.
                Some(e) => { held.ok = false; held.error = Some(e); }
                None => {
                    let mut model = build(&raw);
                    let now = now_ms();
                    if let Ok(mut a) = app.state::<AlertState>().0.lock() { outcome = a.on_read(&model, now); a.mark(&mut model); }
                    *held = Snapshot { ok: true, error: None, read_at: Some(now), dnd_until: None, model };
                }
            }
            // Do Not Disturb ends by itself; the window and the menu show whether it holds.
            let dnd = match app.state::<AlertState>().0.lock() {
                Ok(mut a) => { let was = a.dnd_until; let on = a.quiet(now_ms()); if was.is_some() && !on { drop(a); save_settings(&app); } if on { was } else { None } }
                Err(_) => None,
            };
            held.dnd_until = dnd;
            held.clone()
        };
        // Notifications, sound and flash run after the snapshot lock is released: a slow system call cannot hold it.
        for alert in &outcome.alerts { notify(&app, alert); }
        if outcome.sound { play_sound(); }
        if outcome.flash { flash_tray(app.clone()); }
        if let Ok(menu) = app.state::<DndMenu>().0.lock() {
            if let Some(sub) = menu.as_ref() { let _ = sub.set_text(if snap.dnd_until.is_some() { "Do not disturb: on" } else { "Do not disturb" }); }
        }
        publish(&app, &snap);
        // Wait for the pause, or less when the Refresh button asks; a burst of presses counts as one.
        match wake.recv_timeout(POLL_PAUSE) {
            Ok(()) => while wake.try_recv().is_ok() {},
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(POLL_PAUSE),
        }
    }
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn hide_popover(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("popover") { let _ = w.hide(); }
}

/// Where the popover goes: centred under the menu bar icon, 6 px below it, and inside the icon's screen.
/// All values are physical pixels; `screen` is (left, width) of the monitor the icon is on.
fn popover_x(icon_x: f64, icon_w: f64, pop_w: f64, screen: (f64, f64)) -> f64 {
    let x = icon_x + icon_w / 2.0 - pop_w / 2.0;
    let (left, width) = screen;
    x.min(left + width - pop_w - 8.0).max(left + 8.0)
}

/// A left click on the menu bar icon opens the popover under it, or closes it when it is open.
fn toggle_popover(app: &AppHandle, rect: tauri::Rect) {
    let Some(w) = app.get_webview_window("popover") else { return };
    if w.is_visible().unwrap_or(false) { let _ = w.hide(); return; }
    let scale = w.scale_factor().unwrap_or(1.0);
    let pos: tauri::PhysicalPosition<f64> = rect.position.to_physical(scale);
    let size: tauri::PhysicalSize<f64> = rect.size.to_physical(scale);
    let screen = w
        .monitor_from_point(pos.x, pos.y)
        .ok()
        .flatten()
        .map(|m| (m.position().x as f64, m.size().width as f64))
        .unwrap_or((0.0, f64::MAX / 4.0));
    let x = popover_x(pos.x, size.width, POPOVER_W * scale, screen);
    let _ = w.set_position(tauri::PhysicalPosition::new(x, pos.y + size.height + 6.0 * scale));
    let _ = w.show();
    let _ = w.set_focus();
}

fn main() {
    let (wake_tx, wake_rx) = channel::<()>();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .manage(Shared::default())
        .manage(Wake(Mutex::new(wake_tx)))
        .manage(AlertState(Mutex::new(Alerts::new(true))))
        .manage(DndMenu(Mutex::new(None)))
        .manage(Told::default())
        .invoke_handler(tauri::generate_handler![snapshot, refresh, open_session, open_link, set_dnd, peek, show_board, close_popover])
        .setup(move |app| {
            let handle = app.handle().clone();
            // With a handler registered, the plugin reports a click on a notification (`tap`).
            let clicks = app.handle().clone();
            let _ = app.notification().on_action(move |performed| {
                if performed.action_id() == "tap" { on_note_click(&clicks, performed.notification().map(|n| n.id())); }
            });
            let saved = load_settings(app.handle());
            if let Ok(mut a) = app.state::<AlertState>().0.lock() {
                a.sound_on = saved.sound.unwrap_or(true);
                a.dnd_until = saved.dnd_until.filter(|&until| until > now_ms());
            }
            let sound_on = saved.sound.unwrap_or(true);
            let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
            let open = MenuItem::with_id(app, "open", "Open Pod Board", true, None::<&str>)?;
            let autostart = CheckMenuItem::with_id(app, "autostart", "Start at login", true, autostart_on, None::<&str>)?;
            let on_top = CheckMenuItem::with_id(app, "on_top", "Always on top", true, false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let sound = CheckMenuItem::with_id(app, "sound", "Sound", true, sound_on, None::<&str>)?;
            let dnd = Submenu::with_items(app, "Do not disturb", true, &[
                &MenuItem::with_id(app, "dnd_30", "For 30 minutes", true, None::<&str>)?,
                &MenuItem::with_id(app, "dnd_60", "For 1 hour", true, None::<&str>)?,
                &MenuItem::with_id(app, "dnd_120", "For 2 hours", true, None::<&str>)?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(app, "dnd_off", "Turn off", true, None::<&str>)?,
            ])?;
            if let Ok(mut slot) = app.state::<DndMenu>().0.lock() { *slot = Some(dnd.clone()); }
            let menu = Menu::with_items(app, &[
                &open, &PredefinedMenuItem::separator(app)?, &sound, &dnd, &PredefinedMenuItem::separator(app)?,
                &autostart, &on_top, &PredefinedMenuItem::separator(app)?, &quit,
            ])?;
            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-coffee.png"))?;
            TrayIconBuilder::with_id("pod-board")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Pod Board")
                .menu(&menu)
                // Left click: the popover with the sessions. Right click (or Control-click): this menu.
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                        toggle_popover(tray.app_handle(), rect);
                    }
                })
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => show_main(app),
                    "autostart" => {
                        let manager = app.autolaunch();
                        let want = !manager.is_enabled().unwrap_or(false);
                        let _ = if want { manager.enable() } else { manager.disable() };
                        let _ = autostart.set_checked(manager.is_enabled().unwrap_or(false));
                    }
                    "on_top" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let want = on_top.is_checked().unwrap_or(false);
                            let _ = window.set_always_on_top(want);
                        }
                    }
                    "sound" => {
                        let on = sound.is_checked().unwrap_or(true);
                        if let Ok(mut a) = app.state::<AlertState>().0.lock() { a.sound_on = on; }
                        save_settings(app);
                    }
                    "dnd_30" => apply_dnd(app, 30),
                    "dnd_60" => apply_dnd(app, 60),
                    "dnd_120" => apply_dnd(app, 120),
                    "dnd_off" => apply_dnd(app, 0),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Started at login: stay in the Dock and the menu bar, do not pop a window up.
            if std::env::args().any(|a| a == "--hidden") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            std::thread::spawn(move || poll_loop(handle, wake_rx));
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the window hides it: the app keeps watching and the Dock icon opens it again.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // The popover closes as soon as the owner clicks anywhere else, like a system menu.
            WindowEvent::Focused(false) if window.label() == "popover" => {
                let _ = window.hide();
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Pod Board");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            show_main(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, &event, RunEvent::Ready);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pod-board-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("podbay-dev");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn peek_keeps_the_newest_reply_and_the_prompt_but_never_the_screen() {
        let v = serde_json::json!({
            "prompt": "  fix the build  ",
            "reply": "older reply",
            "peek": "👤 someone@example.com │ the terminal screen",
            "shared": true,
            "activity": [
                {"kind": "reply", "at": 10, "full": "first", "line": "first"},
                {"kind": "prompt", "at": 30, "full": "a prompt", "line": "a prompt"},
                {"kind": "reply", "at": 20, "full": "Shall I push the branch?", "line": "Shall I…"}
            ]
        });
        let p = peek_of(&v);
        assert_eq!(p, Peek { prompt: "fix the build".into(), reply: "Shall I push the branch?".into(), at: Some(20), shared: true });
        assert!(!serde_json::to_string(&p).unwrap().contains("example.com"));
        // No activity: the plain `reply` field, and a machine that does not share its text says so.
        let q = peek_of(&serde_json::json!({"reply": "done", "prompt": null, "shared": false}));
        assert_eq!((q.reply.as_str(), q.prompt.as_str(), q.at, q.shared), ("done", "", None, false));
    }

    #[test]
    fn the_head_and_tail_of_a_long_reply_are_joined() {
        // Overlap, as Pod Bay sends a 700-character reply: 600 head characters, 600 tail characters.
        let whole = format!("Start. {} The middle part. {} Shall I push?", "a".repeat(300), "b".repeat(300));
        let head = format!("{}…", whole.chars().take(600).collect::<String>());
        let tail_part = format!("…{}", whole.chars().skip(whole.chars().count() - 600).collect::<String>());
        assert_eq!(join_reply(&head, &tail_part), whole);
        // No overlap: both pieces with a "…" line between, the end kept.
        assert_eq!(join_reply("Start here and…", "…far later. Shall I push?"), "Start here and\n\n…\n\nfar later. Shall I push?");
        // A short reply is whole in `full`.
        assert_eq!(join_reply("Done.", "Done."), "Done.");
        let v = serde_json::json!({"reply": tail_part, "activity": [{"kind": "reply", "at": 5, "full": head}]});
        assert!(peek_of(&v).reply.ends_with("Shall I push?"));
    }

    #[test]
    fn a_long_reply_keeps_its_end_where_the_question_is() {
        let long = format!("{}Shall I go on?", "x".repeat(5000));
        let cut = tail(&long, PEEK_REPLY_CHARS);
        assert_eq!(cut.chars().count(), PEEK_REPLY_CHARS);
        assert!(cut.starts_with('…') && cut.ends_with("Shall I go on?"));
        assert_eq!(tail("short", 10), "short");
    }

    #[test]
    fn the_menu_bar_counts_finished_sessions_unless_someone_waits() {
        let look = |needs, busy, idle, finished| {
            let mut snap = Snapshot { ok: true, ..Snapshot::default() };
            snap.model.counts = model::Counts { needs, busy, idle, stopped: 0, finished };
            tray_look(&snap).1
        };
        assert_eq!(look(0, 0, 3, 0), "");
        assert_eq!(look(0, 0, 3, 2), "✓2");
        assert_eq!(look(0, 4, 3, 0), "4");
        assert_eq!(look(0, 4, 3, 2), "4 ✓2");
        assert_eq!(look(1, 4, 3, 2), "1", "a wait comes first");
    }

    #[test]
    fn the_popover_sits_under_its_icon_and_stays_on_the_screen() {
        // Icon at x 1000, 40 wide, on a 1440-wide screen: centred under it.
        assert_eq!(popover_x(1000.0, 40.0, 340.0, (0.0, 1440.0)), 850.0);
        // Icon near the right edge: pushed left so the popover keeps 8 px from the edge.
        assert_eq!(popover_x(1420.0, 20.0, 340.0, (0.0, 1440.0)), 1092.0);
        // Icon near the left edge of a second screen that starts at x 1440.
        assert_eq!(popover_x(1450.0, 20.0, 340.0, (1440.0, 1920.0)), 1448.0);
    }

    #[test]
    #[cfg(unix)]
    fn a_remote_read_names_its_subcommand_and_has_its_own_time_limit() {
        let cmd = script("on", "exit 4");
        assert_eq!(read_for(&cmd, &["--on", "U0", "detail", "claude-x-00"], PEEK_TIMEOUT).unwrap_err(), "podbay-dev detail failed");
        let slow = script("slow", "sleep 3; printf '{}'");
        let started = Instant::now();
        assert_eq!(read_for(&slow, &["--on", "U0", "detail", "n"], Duration::from_secs(1)).unwrap_err(), "podbay-dev detail timed out");
        assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
    }

    #[test]
    #[cfg(unix)]
    fn read_parses_json_and_names_the_subcommand_on_failure() {
        assert_eq!(read(&script("ok", "printf '[1,2]'"), &["ls", "--all"]).unwrap(), serde_json::json!([1, 2]));
        assert_eq!(read(&script("bad", "echo secret-stderr >&2; exit 3"), &["ls"]).unwrap_err(), "podbay-dev ls failed");
        assert_eq!(read(&script("junk", "echo not json"), &["machines"]).unwrap_err(), "podbay-dev machines did not return JSON");
    }

    #[test]
    #[cfg(unix)]
    fn a_helper_that_keeps_the_pipe_open_cannot_hang_the_read() {
        // The command prints its JSON and exits, but leaves a child that holds stdout open for 30 s.
        let cmd = script("held", "printf '[]'; (sleep 30) & exit 0");
        let started = Instant::now();
        let got = read(&cmd, &["ls", "--all"]);
        assert!(started.elapsed() < COMMAND_TIMEOUT + Duration::from_secs(2), "took {:?}", started.elapsed());
        assert_eq!(got.unwrap_err(), "podbay-dev ls kept its output open");
    }
}
