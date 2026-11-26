// Pod Board: a small desktop window with every Pod Bay session by state (needs you, busy, idle).
// It only observes. It polls the read-only commands `podbay-dev ls --all`, `checkouts --all`, `machines` and
// `queue ls --all`, and reads this machine's Claude session files and sub-agent files (`local.rs`; read only, no
// hook). It runs `podbay-dev detail` only when the owner opens a session's peek, for that one session,
// and keeps only its last prompt and reply (never the screen). It never sends anything anywhere. A click can open
// a session in the Claude app, or one of its links (a pull request, an issue, an artifact) in the browser.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alerts;
mod local;
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

/// Open a session of the Claude desktop app with `claude://code/continue?session=local_<uuid>`. The link form is the
/// one the desktop app's own handler accepts (read in its bundle, 2026-11-26: `/continue` with a `session` that is
/// `last` or `local_` and letters, digits and `-`); opening one was not run here (README, Limits). Only macOS has the app.
fn open_desktop_app(id: &str) -> Result<(), String> {
    if !local::desktop_id(id) {
        return Err("not a Claude desktop session id".into());
    }
    #[cfg(target_os = "macos")]
    {
        let url = format!("claude://code/continue?session={id}");
        spawn_reaped(Command::new("open").arg(&url)).map_err(|_| "could not open the session".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    Err("the Claude desktop app is not available here".into())
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

/// Open a desktop-app session. Only an id the snapshot holds right now passes, so the page cannot make the app open
/// anything else.
#[tauri::command]
fn open_desktop(app: AppHandle, id: String) -> Result<(), String> {
    let known = {
        let state = app.state::<Shared>();
        let held = state.0.lock().map_err(|_| "no data".to_string())?;
        held.model.sessions.iter().any(|s| s.desktop_id == id)
    };
    if !known { return Err("not a desktop session of the board".into()); }
    open_desktop_app(&id)?;
    looked(&app, &format!("desktop|{id}"));
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

/// The PATH of the `podbay-dev` child. A desktop app starts with only `/usr/bin:/bin:/usr/sbin:/sbin`, and
/// `podbay-dev` needs `tmux` (Homebrew) to list this machine's sessions, so the directories where it and the
/// tools it uses live come after the app's own, without repeats (issue #174).
fn child_path(path: Option<std::ffi::OsString>, home: Option<std::ffi::OsString>) -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = path.as_ref().map(|p| std::env::split_paths(p).collect()).unwrap_or_default();
    let mut more: Vec<PathBuf> = home.map(|h| PathBuf::from(h).join(".local").join("bin")).into_iter().collect();
    more.push(PathBuf::from("/usr/local/bin"));
    more.push(PathBuf::from("/opt/homebrew/bin"));
    for d in more {
        if !dirs.contains(&d) { dirs.push(d); }
    }
    // A directory that cannot be joined (a `:` in the home folder) leaves the inherited PATH as it was.
    std::env::join_paths(dirs).unwrap_or_else(|_| path.unwrap_or_default())
}

/// Is this "No route to host" (errno 65) on a private (local network) address? macOS gives it when Local Network
/// access is not allowed for the app (the `podbay-dev` it starts inherits that), and also when the host is off or not
/// on the subnet, so it says the machine cannot be reached, not why (issue #174). Any other error is some other problem.
fn no_route_on_lan(addr: &str, os_error: Option<i32>) -> bool {
    const EHOSTUNREACH: i32 = 65;
    let Ok(sock) = addr.parse::<std::net::SocketAddr>() else { return false };
    let local = match sock.ip() {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_link_local(),
        std::net::IpAddr::V6(_) => false,
    };
    local && os_error == Some(EHOSTUNREACH)
}

/// The machines `ls --all` returned rows for (a result that is a list), by name.
fn listed_machines(ls: &Value) -> Vec<String> {
    ls.as_array()
        .map(|l| l.iter().filter(|m| m["result"].is_array()).filter_map(|m| m["machine"].as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// The names of the machines that Pod Bay knows an address for and that this app gets "No route to host" from. The
/// cause is not known: macOS Local Network access for the app, or the host being off, give the same error. A machine
/// that returned rows is not probed. The probes run side by side, so a few silent hosts cost one timeout, not one each.
fn unreachable_machines(machines: &Value, listed: &[String]) -> Vec<String> {
    let Some(list) = machines.as_array() else { return vec![] };
    let todo: Vec<(String, String)> = list
        .iter()
        .filter(|m| m["me"] != Value::Bool(true))
        .filter_map(|m| Some((m["machine"].as_str()?.to_string(), m["addr"].as_str()?.to_string())))
        .filter(|(name, _)| !listed.contains(name))
        .collect();
    std::thread::scope(|scope| {
        let probes: Vec<_> = todo
            .iter()
            .map(|(name, addr)| {
                scope.spawn(move || {
                    let sock = addr.parse().ok()?;
                    let err = std::net::TcpStream::connect_timeout(&sock, Duration::from_secs(2)).err()?;
                    no_route_on_lan(addr, err.raw_os_error()).then(|| name.clone())
                })
            })
            .collect();
        probes.into_iter().filter_map(|p| p.join().ok().flatten()).collect()
    })
}

/// `~/.claude`, where Claude Code keeps its session files.
fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude"))
}

/// The Claude desktop app's data folder (`~/Library/Application Support/Claude`), on macOS only: that is where it
/// keeps one file per session (issue #176). No other system has the app's folder in a place that was seen.
fn desktop_dir() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") { return None; }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Claude"))
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
        .env("PATH", child_path(std::env::var_os("PATH"), std::env::var_os("HOME")))
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
        // Pod Bay could not be read, but this machine's own sessions still count (issue #158): a wait shows its number.
        (include_bytes!("../icons/tray-triangle-alert.png"), if c.needs > 0 { c.needs.to_string() } else { String::new() })
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

/// The Dock shows the icon it cached for the bundle, and a Dock that pinned Pod Board before the Pod Cat icon (#136) kept
/// the old one: a new `icon.icns`, `lsregister -f` and a Dock restart did not change it (issue #159, checked on macOS 27).
/// So the running app hands the Dock its icon itself, from the same picture as the bundle icon.
#[cfg(target_os = "macos")]
fn dock_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    // `setup` runs on the main thread; anywhere else, leave the Dock as it is.
    let Some(mtm) = MainThreadMarker::new() else { return };
    let data = NSData::with_bytes(include_bytes!("../icons/icon.png"));
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(not(target_os = "macos"))]
fn dock_icon() {}

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

/// Turn one read into the snapshot and the alerts. When `ls` failed (`error`), the Pod Bay part is the last read
/// (`raw.ls` keeps it) and marked as not read, so it raises nothing; this machine's terminal sessions (local ones that
/// Pod Bay does not list) are fresh and alert as usual (issue #158). A session Pod Bay lists stays as last read. The snapshot then says why the read failed and keeps the time of the last good read.
fn apply_read(held: &mut Snapshot, raw: &mut Raw, error: Option<String>, alerts: &mut Alerts, now: u64) -> alerts::Outcome {
    raw.stale = error.is_some();
    let mut model = build(raw);
    let outcome = alerts.on_read(&model, now);
    alerts.mark(&mut model);
    let read_at = if error.is_some() { held.read_at } else { Some(now) };
    *held = Snapshot { ok: error.is_none(), error, read_at, dnd_until: held.dnd_until, model };
    outcome
}

fn poll_loop(app: AppHandle, wake: Receiver<()>) {
    let cmd = find_podbay();
    let mut raw = Raw::default();
    let mut cache = local::Cache::default();
    let mut desktop_cache = local::DesktopCache::default();
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
                    raw.unreachable = unreachable_machines(&raw.machines, &listed_machines(&raw.ls));
                    if let Ok(v) = read(&cmd, &["queue", "ls", "--all"]) { raw.queue = v; }
                }
                n = n.wrapping_add(1);
            }
            Err(e) => error = Some(e),
        }
        // This machine's own Claude sessions: a few small files, read on every poll.
        raw.local = claude_dir().map(|d| local::read_with(&d, now_ms(), &local::alive, &mut cache)).unwrap_or_default();
        raw.desktop = desktop_dir().map(|d| local::read_desktop_with(&d, &mut desktop_cache)).unwrap_or_default();
        raw.now = now_ms();

        let outcome;
        let snap = {
            let state = app.state::<Shared>();
            let mut held = match state.0.lock() { Ok(g) => g, Err(p) => p.into_inner() };
            {
                let alerts = app.state::<AlertState>();
                let mut a = match alerts.0.lock() { Ok(g) => g, Err(p) => p.into_inner() };
                outcome = apply_read(&mut held, &mut raw, error, &mut a, now_ms());
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

/// Does a mouse click at `point` close the popover? Yes when it is shown and the click is outside its `frame`
/// (x, y, width, height; both in one space, the macOS global points). The focus plays no part: on the laptop display
/// the popover never becomes the key window while the board's window is on the other display, so `Focused(false)`
/// never fires there (issue #170).
fn click_hides_popover(visible: bool, frame: (f64, f64, f64, f64), point: (f64, f64)) -> bool {
    let (x, y, w, h) = frame;
    // A frame holds its low edges, not its high ones.
    visible && !(point.0 >= x && point.0 < x + w && point.1 >= y && point.1 < y + h)
}

/// Where the mouse is, in macOS global points. The same space as a window frame.
#[cfg(target_os = "macos")]
fn mouse_point() -> (f64, f64) {
    let m = objc2_app_kit::NSEvent::mouseLocation();
    (m.x, m.y)
}

#[cfg(target_os = "macos")]
fn note_closed_by_click() {
    if let Ok(mut g) = POPOVER_CLOSED_BY_CLICK.lock() { *g = Some((Instant::now(), mouse_point())); }
}

/// macOS: close the popover on a click anywhere else, like a system menu. A global monitor sees the clicks that go
/// to other apps (the desktop, another display's menu bar); a local one sees the clicks on the board's window, the
/// only window of this app that is not the popover or the menu bar icon.
#[cfg(target_os = "macos")]
fn close_popover_on_click_away(app: &AppHandle) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSEventMask, NSWindow};
    use std::ptr::NonNull;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let mask = NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;
    fn ns_of(app: &AppHandle, label: &str) -> Option<usize> {
        app.get_webview_window(label).and_then(|w| w.ns_window().ok()).map(|p| p as usize)
    }
    // Is the click outside the popover? Read the window's own frame: the same space as the mouse location.
    fn outside(app: &AppHandle) -> bool {
        let Some(p) = ns_of(app, "popover") else { return false };
        let visible = app.get_webview_window("popover").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        // SAFETY: `ns_window` is this app's own NSWindow, alive while the webview window is; used on the main thread.
        let f = unsafe { &*(p as *const NSWindow) }.frame();
        let m = NSEvent::mouseLocation();
        click_hides_popover(visible, (f.origin.x, f.origin.y, f.size.width, f.size.height), (m.x, m.y))
    }
    let other = app.clone();
    let global = RcBlock::new(move |_: NonNull<NSEvent>| {
        if outside(&other) { hide_popover(&other); note_closed_by_click(); }
    });
    let _ = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &global);
    let this = app.clone();
    let local = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // Only a click on the board's window counts here. A click on the popover, or on this app's menu bar icon
        // (whose own click toggles the popover), is left alone.
        let on_board = MainThreadMarker::new()
            .and_then(|mtm| unsafe { event.as_ref() }.window(mtm))
            .map(|w| &*w as *const NSWindow as usize)
            .is_some_and(|w| Some(w) == ns_of(&this, "main"));
        if on_board && outside(&this) { hide_popover(&this); note_closed_by_click(); }
        event.as_ptr()
    });
    // SAFETY: the block returns the event it was given, a valid pointer.
    let _ = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &local) };
    // The monitors live as long as the app.
    std::mem::forget(global);
    std::mem::forget(local);
    let _ = mtm;
}

#[cfg(not(target_os = "macos"))]
fn mouse_point() -> (f64, f64) { (0.0, 0.0) }

#[cfg(not(target_os = "macos"))]
fn close_popover_on_click_away(_app: &AppHandle) {}

/// Where the popover goes: centred under the menu bar icon, 6 px below it, and inside the icon's screen.
/// All values are in one unit (here logical points); `screen` is (left, width) of the monitor the icon is on.
fn popover_x(icon_x: f64, icon_w: f64, pop_w: f64, screen: (f64, f64)) -> f64 {
    let x = icon_x + icon_w / 2.0 - pop_w / 2.0;
    let (left, width) = screen;
    x.min(left + width - pop_w - 8.0).max(left + 8.0)
}

/// One monitor as tao reports it: position and size in physical pixels, and its scale factor.
#[derive(Clone, Copy, Debug)]
struct Mon { x: f64, y: f64, w: f64, h: f64, scale: f64 }

/// Where the popover goes, in logical points (the space a `LogicalPosition` takes).
/// `icon` is the tray rect (x, y, w, h) as tray-icon reports it on macOS: logical points times the
/// scale of the icon's own screen. The click point (`cursor`, logical points) names that screen: the
/// monitor whose logical bounds hold it. Only then is the rect divided by that monitor's scale, so
/// two screens with different scales cannot be mixed up. The popover is centred under the icon,
/// 6 points below it, and kept 8 points inside that screen.
fn popover_place(icon: (f64, f64, f64, f64), cursor: (f64, f64), pop_w: f64, mons: &[Mon]) -> Option<(f64, f64)> {
    let (ix, iy, iw, ih) = icon;
    let m = mons.iter().find(|m| {
        let (left, top) = (m.x / m.scale, m.y / m.scale);
        cursor.0 >= left && cursor.0 < left + m.w / m.scale && cursor.1 >= top && cursor.1 < top + m.h / m.scale
    })?;
    let x = popover_x(ix / m.scale, iw / m.scale, pop_w, (m.x / m.scale, m.w / m.scale));
    Some((x, iy / m.scale + ih / m.scale + 6.0))
}

/// The click point when the system cannot give it: the middle of the icon, in logical points. The icon rect is
/// logical points times the scale of its own screen, so a screen can hold it only when the rect divided by that
/// screen's scale falls in that screen's logical bounds (issue #158: the rect itself was matched against logical
/// bounds, which could name another screen). Two screens with different scales can both fit; then the primary
/// screen (at `primary`, its position as tao gives it) wins, else the first that fits.
fn icon_point(icon: (f64, f64, f64, f64), mons: &[Mon], primary: Option<(f64, f64)>) -> Option<(f64, f64)> {
    let (ix, iy, iw, ih) = icon;
    let mid = |m: &Mon| ((ix + iw / 2.0) / m.scale, (iy + ih / 2.0) / m.scale);
    let fits = |m: &&Mon| {
        let ((x, y), (left, top)) = (mid(m), (m.x / m.scale, m.y / m.scale));
        x >= left && x < left + m.w / m.scale && y >= top && y < top + m.h / m.scale
    };
    let mut hits = mons.iter().filter(fits);
    let first = hits.next()?;
    let pick = std::iter::once(first).chain(hits).find(|m| Some((m.x, m.y)) == primary).unwrap_or(first);
    Some(mid(pick))
}

/// The menu bar icon the popover was last opened for (its tray rect's top left), and when a click elsewhere last
/// closed it. A click on the icon is a mouse-down, which the click monitor sees as a click away, then a mouse-up,
/// which toggles: the two must not undo each other.
static POPOVER_ICON: Mutex<Option<(f64, f64)>> = Mutex::new(None);
static POPOVER_CLOSED_BY_CLICK: Mutex<Option<(Instant, (f64, f64))>> = Mutex::new(None);
/// A mouse-up this soon after a click closed the popover, and with the mouse still where that click was, belongs to
/// that click. Without it the icon's own click would close the popover on mouse-down and reopen it on mouse-up.
const CLICK_PAIR_MS: u128 = 600;
const CLICK_PAIR_MOVED: f64 = 4.0;

/// Two tray rects are the same icon when their top left is close: the icon's width follows its count text, and
/// another menu bar item can push it sideways. Another display's icon is a whole screen away.
fn same_icon(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 300.0 && (a.1 - b.1).abs() < 20.0
}

#[derive(Debug, PartialEq)]
enum Toggle { Hide, Stay, Show }

/// What a click on a menu bar icon does: close the popover that is open under this icon, leave it closed when this
/// click's own mouse-down just closed it, otherwise open it under this icon (also moving it from the other display's).
/// `closed_by_click` is, when a click elsewhere closed it, how many ms ago and how many points the mouse moved since.
fn toggle_action(visible: bool, same_icon: bool, closed_by_click: Option<(u128, f64)>) -> Toggle {
    match (visible, same_icon) {
        (true, true) => Toggle::Hide,
        (true, false) => Toggle::Show,
        (false, true) if closed_by_click.is_some_and(|(ms, moved)| ms < CLICK_PAIR_MS && moved < CLICK_PAIR_MOVED) => Toggle::Stay,
        (false, _) => Toggle::Show,
    }
}

/// A left click on the menu bar icon opens the popover under it, or closes it when it is open under that icon.
fn toggle_popover(app: &AppHandle, rect: tauri::Rect) {
    let Some(w) = app.get_webview_window("popover") else { return };
    let at = rect.position.to_physical::<f64>(1.0);
    let at = (at.x, at.y);
    let same = POPOVER_ICON.lock().ok().and_then(|g| *g).is_some_and(|i| same_icon(i, at));
    let closed = POPOVER_CLOSED_BY_CLICK.lock().ok().and_then(|g| *g).map(|(t, at)| {
        let now = mouse_point();
        (t.elapsed().as_millis(), (now.0 - at.0).hypot(now.1 - at.1))
    });
    let action = toggle_action(w.is_visible().unwrap_or(false), same, closed);
    // The stamp belongs to one click: spent now, whatever the action.
    if let Ok(mut g) = POPOVER_CLOSED_BY_CLICK.lock() { *g = None; }
    match action {
        Toggle::Hide => { let _ = w.hide(); return; }
        Toggle::Stay => return,
        Toggle::Show => {}
    }
    if let Ok(mut g) = POPOVER_ICON.lock() { *g = Some(at); }
    let mons: Vec<Mon> = w
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| Mon {
            x: m.position().x as f64,
            y: m.position().y as f64,
            w: m.size().width as f64,
            h: m.size().height as f64,
            scale: m.scale_factor(),
        })
        .collect();
    let icon = (rect.position.to_physical::<f64>(1.0), rect.size.to_physical::<f64>(1.0));
    let icon = (icon.0.x, icon.0.y, icon.1.width, icon.1.height);
    // tao reports the cursor as logical points times the primary screen's scale.
    let main_mon = app.primary_monitor().ok().flatten();
    let primary = main_mon.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let main_at = main_mon.as_ref().map(|m| (m.position().x as f64, m.position().y as f64));
    let cursor = app.cursor_position().ok().map(|c| (c.x / primary, c.y / primary)).or_else(|| icon_point(icon, &mons, main_at));
    let placed = cursor.and_then(|c| popover_place(icon, c, POPOVER_W, &mons));
    // No monitor holds the click (should not happen): open under the icon, unclamped, at the popover's scale.
    let (x, y) = placed.unwrap_or_else(|| {
        let s = w.scale_factor().unwrap_or(1.0);
        (icon.0 / s + icon.2 / s / 2.0 - POPOVER_W / 2.0, icon.1 / s + icon.3 / s + 6.0)
    });
    let _ = w.set_position(tauri::LogicalPosition::new(x, y));
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
        .invoke_handler(tauri::generate_handler![snapshot, refresh, open_session, open_desktop, open_link, set_dnd, peek, show_board, close_popover])
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

            dock_icon();
            close_popover_on_click_away(app.handle());
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
    fn a_failed_read_shows_the_warning_icon_and_the_number_of_waits() {
        // Issue #158 bug 2: a local wait raised during a failed `ls` gets its badge.
        let mut snap = Snapshot { ok: false, error: Some("podbay-dev ls failed".into()), ..Snapshot::default() };
        snap.model.counts = model::Counts { needs: 2, busy: 1, idle: 0, stopped: 0, finished: 0 };
        let (icon, title) = tray_look(&snap);
        assert_eq!((icon, title.as_str()), (&include_bytes!("../icons/tray-triangle-alert.png")[..], "2"));
        snap.model.counts.needs = 0;
        assert_eq!(tray_look(&snap).1, "");
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
    fn the_popover_lands_under_the_icon_on_each_of_two_displays_with_different_scale() {
        // Retina laptop (scale 2, 1440x900 points) at the origin; external (scale 1, 1920x1080 points) to its right.
        // tao: monitor position = logical origin x scale; the tray rect = logical x the icon screen's scale;
        // the click point is given here in logical points.
        let mons = [
            Mon { x: 0.0, y: 0.0, w: 2880.0, h: 1800.0, scale: 2.0 },
            Mon { x: 1440.0, y: 0.0, w: 1920.0, h: 1080.0, scale: 1.0 },
        ];
        // Icon on the laptop: logical x 1000, 24x24 points -> physical x2.
        let (x, y) = popover_place((2000.0, 0.0, 48.0, 48.0), (1012.0, 10.0), 340.0, &mons).unwrap();
        assert_eq!((x, y), (1000.0 + 12.0 - 170.0, 24.0 + 6.0));
        // Icon on the external display: logical x 2800, 22x22 points -> physical x1.
        // Its rect read at the laptop's scale (x 1400) would also fall on the laptop: the click point decides.
        let (x, y) = popover_place((2800.0, 0.0, 22.0, 22.0), (2811.0, 10.0), 340.0, &mons).unwrap();
        assert_eq!((x, y), (2800.0 + 11.0 - 170.0, 22.0 + 6.0));
        // Icon at the far right of the laptop: kept inside that display (1440 points wide).
        let (x, _) = popover_place((2860.0, 0.0, 48.0, 48.0), (1442.0 - 12.0, 10.0), 340.0, &mons).unwrap();
        assert_eq!(x, 1440.0 - 340.0 - 8.0);
        // Icon at the far right of the external display (logical right edge 3360).
        let (x, _) = popover_place((3350.0, 0.0, 22.0, 22.0), (3355.0, 10.0), 340.0, &mons).unwrap();
        assert_eq!(x, 3360.0 - 340.0 - 8.0);
        // The external display on the left of the laptop (negative origin) works the same way.
        let left = [
            Mon { x: -1920.0, y: 0.0, w: 1920.0, h: 1080.0, scale: 1.0 },
            Mon { x: 0.0, y: 0.0, w: 2880.0, h: 1800.0, scale: 2.0 },
        ];
        let (x, _) = popover_place((-500.0, 0.0, 22.0, 22.0), (-489.0, 10.0), 340.0, &left).unwrap();
        assert_eq!(x, -500.0 + 11.0 - 170.0);
    }

    #[test]
    fn one_display_is_unchanged() {
        let mons = [Mon { x: 0.0, y: 0.0, w: 2880.0, h: 1800.0, scale: 2.0 }];
        let (x, y) = popover_place((2000.0, 0.0, 48.0, 48.0), (1012.0, 10.0), 340.0, &mons).unwrap();
        assert_eq!((x, y), (1000.0 + 12.0 - 170.0, 30.0));
        assert!(popover_place((2000.0, 0.0, 48.0, 48.0), (1012.0, 10.0), 340.0, &[]).is_none());
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

    #[test]
    fn without_a_click_point_the_icon_rect_is_matched_with_each_screens_own_scale() {
        // Issue #158 bug 7. The same screens as above: the laptop (scale 2) is the primary one.
        let mons = [
            Mon { x: 0.0, y: 0.0, w: 2880.0, h: 1800.0, scale: 2.0 },
            Mon { x: 1440.0, y: 0.0, w: 1920.0, h: 1080.0, scale: 1.0 },
        ];
        // Icon on the laptop at logical x 1000 (rect x 2000). Read as a logical point, 2000 falls on the external
        // display: the popover went there. Matched with each screen's scale, the primary laptop holds it.
        let icon = (2000.0, 0.0, 48.0, 48.0);
        let click = icon_point(icon, &mons, Some((0.0, 0.0))).unwrap();
        assert_eq!(click, (1012.0, 12.0));
        assert_eq!(popover_place(icon, click, 340.0, &mons).unwrap(), (1000.0 + 12.0 - 170.0, 24.0 + 6.0));
        // Only the external display can hold this rect (3300 / 2 is past the laptop's 1440 points).
        let icon = (3300.0, 0.0, 22.0, 22.0);
        let click = icon_point(icon, &mons, Some((0.0, 0.0))).unwrap();
        assert_eq!(click, (3311.0, 11.0));
        assert_eq!(popover_place(icon, click, 340.0, &mons).unwrap().0, 3360.0 - 340.0 - 8.0);
        // A rect no screen holds gives no point (the caller then opens under the icon, unclamped).
        assert!(icon_point((9000.0, 0.0, 22.0, 22.0), &mons, None).is_none());
    }

    #[test]
    fn a_click_outside_the_popover_closes_it_whichever_window_has_the_focus() {
        // Issue #170: on the laptop display the popover opens but the main window (on the other display) stays the
        // key window, so the popover never sees `Focused(false)`. The click itself decides, not the focus.
        // Frame and point are in one space (macOS global points, origin bottom left): x, y, width, height.
        let frame = (707.0, 524.0, 340.0, 333.0);
        // A click on the desktop of the same screen, on the other display, and on or past each high edge.
        assert!(click_hides_popover(true, frame, (300.0, 400.0)));
        assert!(click_hides_popover(true, frame, (500.0, 1500.0)));
        assert!(click_hides_popover(true, frame, (706.9, 600.0)));
        assert!(click_hides_popover(true, frame, (1047.0, 600.0)));
        assert!(click_hides_popover(true, frame, (800.0, 857.0)));
        assert!(click_hides_popover(true, frame, (800.0, 523.9)));
        // A click inside the popover keeps it; a hidden popover has nothing to close.
        assert!(!click_hides_popover(true, frame, (800.0, 600.0)));
        assert!(!click_hides_popover(true, frame, (707.0, 524.0)));
        assert!(!click_hides_popover(false, frame, (300.0, 400.0)));
    }

    #[test]
    fn a_click_on_a_menu_bar_icon_opens_closes_or_moves_the_popover() {
        let (laptop, external) = ((864.0, 3.0), (1386.0, -1437.0));
        // Closed: a click opens it.
        assert_eq!(toggle_action(false, false, None), Toggle::Show);
        // Open, same icon: the click closes it. Open for the other display's icon: it moves there (not just closes).
        assert_eq!(toggle_action(true, true, None), Toggle::Hide);
        assert_eq!(toggle_action(true, false, None), Toggle::Show);
        // The mouse-down of this same click already closed it (the click monitor): the mouse-up must not reopen it.
        assert_eq!(toggle_action(false, true, Some((80, 0.0))), Toggle::Stay);
        // A click on the other icon closed it on mouse-down too, but that click opens it under its own icon.
        assert_eq!(toggle_action(false, false, Some((80, 0.0))), Toggle::Show);
        // Closed long ago, or by a click somewhere else a moment ago (the mouse moved to the icon): open.
        assert_eq!(toggle_action(false, true, Some((5000, 0.0))), Toggle::Show);
        assert_eq!(toggle_action(false, true, Some((200, 400.0))), Toggle::Show);
        // The icon's width changes with its count text and another item can push it sideways, so its left edge
        // moves; the other display's icon is a whole screen away.
        assert!(same_icon(laptop, (850.0, 3.0)));
        assert!(same_icon(laptop, (600.0, 3.0)));
        assert!(!same_icon(laptop, external));
    }

    #[test]
    fn the_child_path_holds_where_podbay_and_tmux_live_without_repeats() {
        use std::ffi::OsString;
        let os = |s: &str| Some(OsString::from(s));
        // Issue #174: started from Finder the app has only the system directories, so `podbay-dev` found no tmux.
        assert_eq!(
            child_path(os("/usr/bin:/bin:/usr/sbin:/sbin"), os("/Users/x")),
            OsString::from("/usr/bin:/bin:/usr/sbin:/sbin:/Users/x/.local/bin:/usr/local/bin:/opt/homebrew/bin")
        );
        // What the app already has stays first and is not repeated.
        assert_eq!(
            child_path(os("/opt/homebrew/bin:/usr/bin"), os("/Users/x")),
            OsString::from("/opt/homebrew/bin:/usr/bin:/Users/x/.local/bin:/usr/local/bin")
        );
        assert_eq!(child_path(None, None), OsString::from("/usr/local/bin:/opt/homebrew/bin"));
        // A home with a ':' cannot be joined into a PATH: keep the inherited one, never an empty one.
        assert_eq!(child_path(os("/usr/bin:/bin"), os("/Users/a:b")), OsString::from("/usr/bin:/bin"));
    }

    #[test]
    #[cfg(unix)]
    fn the_podbay_child_gets_the_longer_path() {
        let got = read(&script("path", "printf '[\"%s\"]' \"$PATH\""), &["ls"]).unwrap();
        let path = got[0].as_str().unwrap().to_string();
        assert!(path.contains("/opt/homebrew/bin") && path.contains("/usr/local/bin"), "the child PATH is {path}");
    }

    #[test]
    fn only_machines_that_returned_rows_count_as_listed() {
        let ls = serde_json::json!([{"machine": "M0", "result": []}, {"machine": "U0", "result": "unreachable"}, {"machine": "U1"}]);
        assert_eq!(listed_machines(&ls), vec!["M0".to_string()]);
        assert!(listed_machines(&serde_json::json!("x")).is_empty());
    }

    #[test]
    fn only_no_route_to_a_local_address_is_unreachable_from_the_app() {
        // errno 65 (EHOSTUNREACH): what macOS gives a connection that "Local Network" access does not allow.
        assert!(no_route_on_lan("192.168.1.218:7880", Some(65)));
        assert!(no_route_on_lan("10.0.0.5:7880", Some(65)));
        assert!(no_route_on_lan("172.16.3.4:7880", Some(65)));
        // Refused, timed out, reached, or not a local address: not this problem.
        assert!(!no_route_on_lan("192.168.1.218:7880", Some(61)));
        assert!(!no_route_on_lan("192.168.1.218:7880", Some(60)));
        assert!(!no_route_on_lan("192.168.1.218:7880", None));
        assert!(!no_route_on_lan("8.8.8.8:53", Some(65)));
        assert!(!no_route_on_lan("not an address", Some(65)));
    }

    fn term(status: &str, waiting: &str) -> local::Local {
        local::Local { rc_id: "session_01Term".into(), entrypoint: "cli".into(), interactive: true, status: status.into(),
            waiting_for: waiting.into(), title: "Term".into(), project: "alpha".into(), since: 1, ..Default::default() }
    }

    #[test]
    fn a_local_session_that_starts_waiting_while_ls_fails_still_alerts() {
        // Issue #158 bug 2: a failed `podbay-dev ls --all` froze the local sessions and their alerts.
        let pod_wait = serde_json::json!({"title": "Pod wait", "name": "claude-x-01", "project": "x", "context": "x/x@01",
            "status": "waiting", "waiting_for": "input needed", "remote_url": "https://claude.ai/code/session_01PodWait"});
        let mut raw = Raw {
            ls: serde_json::json!([{"machine": "UA", "result": [pod_wait]}]),
            machines: serde_json::json!([{"machine": "UA", "me": true, "online": true}]),
            local: vec![term("idle", "")],
            ..Raw::default()
        };
        let (mut held, mut alerts) = (Snapshot::default(), Alerts::new(true));
        let first = apply_read(&mut held, &mut raw, None, &mut alerts, 1_000);
        assert!(first.alerts.is_empty() && held.ok && held.read_at == Some(1_000));

        // `ls` fails; the terminal session starts waiting. Long after, so a reminder for Pod Bay's wait would be due.
        let later = 1_000 + alerts::REMIND_AFTER_MS + 1;
        raw.local = vec![term("waiting", "input needed")];
        let out = apply_read(&mut held, &mut raw, Some("podbay-dev ls failed".into()), &mut alerts, later);
        let titles: Vec<&str> = out.alerts.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles, vec!["Term needs you"], "the local wait alerts; Pod Bay's last read raises nothing");
        assert!(out.sound && out.flash, "sound and the icon flash");
        assert!(!held.ok && held.error.as_deref() == Some("podbay-dev ls failed") && held.read_at == Some(1_000));
        let term_row = held.model.sessions.iter().find(|s| s.title == "Term").expect("the card is there");
        assert_eq!((term_row.state, term_row.source), ("needs", "terminal"));
        assert!(held.model.sessions.iter().any(|s| s.title == "Pod wait"), "the last Pod Bay sessions stay on the board");
        assert_eq!(held.model.counts.needs, 2, "the badge counts the new wait");

        // `ls` is back: Pod Bay's wait is not announced as new (its reminder, now due, may go out).
        let back = apply_read(&mut held, &mut raw, None, &mut alerts, later + 1_000);
        let titles: Vec<&str> = back.alerts.iter().map(|a| a.title.as_str()).collect();
        assert!(titles.iter().all(|t| !t.ends_with(" needs you")), "{titles:?}");
        assert!(held.ok);
    }

    #[test]
    fn a_failed_first_read_does_not_make_the_first_good_read_announce_every_wait() {
        // Review of #158, F1: the first good Pod Bay read only learns what already waits, even after a failed one.
        let waits: Vec<serde_json::Value> = (1..=3).map(|i| serde_json::json!({"title": format!("0{i}"), "name": format!("claude-x-0{i}"),
            "project": "x", "context": format!("x/x@0{i}"), "status": "waiting", "waiting_for": "input needed",
            "remote_url": format!("https://claude.ai/code/session_01Wait{i}")})).collect();
        let mut raw = Raw { machines: serde_json::json!([{"machine": "UA", "me": true, "online": true}]), ..Raw::default() };
        let (mut held, mut alerts) = (Snapshot::default(), Alerts::new(true));
        let failed = apply_read(&mut held, &mut raw, Some("podbay-dev ls failed".into()), &mut alerts, 1_000);
        assert!(failed.alerts.is_empty());
        raw.ls = serde_json::json!([{"machine": "UA", "result": waits}]);
        let good = apply_read(&mut held, &mut raw, None, &mut alerts, 4_000);
        assert!(good.alerts.is_empty() && !good.sound && !good.flash, "{:?}", good.alerts.iter().map(|a| &a.title).collect::<Vec<_>>());
        // A wait that starts after that is told.
        raw.local = vec![term("waiting", "input needed")];
        let next = apply_read(&mut held, &mut raw, None, &mut alerts, 7_000);
        assert_eq!(next.alerts.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), vec!["Term needs you"]);
    }

    #[test]
    fn a_terminal_session_that_waits_again_during_one_ls_outage_alerts_again() {
        // Review of #158, F2: an ended local wait was kept while this machine counted as not read.
        let mut raw = Raw {
            ls: serde_json::json!([{"machine": "UA", "result": []}]),
            machines: serde_json::json!([{"machine": "UA", "me": true, "online": true}]),
            local: vec![term("idle", "")],
            ..Raw::default()
        };
        let (mut held, mut alerts) = (Snapshot::default(), Alerts::new(true));
        apply_read(&mut held, &mut raw, None, &mut alerts, 1_000);
        let fail = || Some("podbay-dev ls failed".to_string());
        let mut told = Vec::new();
        for (i, (status, waiting)) in [("waiting", "input needed"), ("busy", ""), ("waiting", "permission prompt")].into_iter().enumerate() {
            raw.local = vec![term(status, waiting)];
            let out = apply_read(&mut held, &mut raw, fail(), &mut alerts, 100_000 * (i as u64 + 1));
            told.extend(out.alerts.into_iter().map(|a| a.title));
        }
        assert_eq!(told, vec!["Term needs you", "Term needs you"]);
    }
}
