// Pod Board: a small desktop window with every Pod Bay session by state (needs you, busy, idle).
// It only observes. It runs the read-only commands `podbay-dev ls --all`, `checkouts --all`, `machines` and
// `queue ls --all`; it never calls `podbay-dev detail` and never sends anything anywhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alerts;
mod model;

use alerts::Alerts;
use model::{build, Model, Raw};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;

const POLL_PAUSE: Duration = Duration::from_secs(3); // one `ls --all` takes about 1.3 s
const SLOW_EVERY: u32 = 10; // checkouts, machines and the queue change slowly
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
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
#[tauri::command]
fn open_session(id: String) -> Result<(), String> {
    if id.is_empty() || model::session_id(&format!("https://claude.ai/code/{id}")) != id {
        return Err("not a Claude session id".into());
    }
    #[cfg(target_os = "macos")]
    let (opener, url) = ("open", format!("claude://code/{id}"));
    #[cfg(not(target_os = "macos"))]
    let (opener, url) = ("xdg-open", format!("https://claude.ai/code/{id}"));
    spawn_reaped(Command::new(opener).arg(&url)).map_err(|_| "could not open the session".to_string())
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
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { "podbay-dev not found".to_string() } else { format!("podbay-dev {} did not start", args[0]) })?;
    let mut out = child.stdout.take().ok_or_else(|| format!("podbay-dev {} gave no output", args[0]))?;
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
            Ok(None) if started.elapsed() > COMMAND_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("podbay-dev {} timed out", args[0]));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => return Err(format!("podbay-dev {} failed", args[0])),
        }
    };
    let left = COMMAND_TIMEOUT.saturating_sub(started.elapsed()).max(Duration::from_secs(1));
    let text = rx.recv_timeout(left).map_err(|_| format!("podbay-dev {} kept its output open", args[0]))?;
    if !status.success() {
        return Err(format!("podbay-dev {} failed", args[0]));
    }
    serde_json::from_str(&text).map_err(|_| format!("podbay-dev {} did not return JSON", args[0]))
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
/// Icons: Phosphor Icons, duotone, drawn black so macOS can tint them as template images.
fn tray_look(snap: &Snapshot) -> (&'static [u8], String) {
    let c = snap.model.counts;
    if !snap.ok {
        (include_bytes!("../icons/tray-warning-circle.png"), String::new())
    } else if c.needs > 0 {
        (include_bytes!("../icons/tray-hand-palm.png"), c.needs.to_string())
    } else if c.busy > 0 {
        (include_bytes!("../icons/tray-lightning.png"), c.busy.to_string())
    } else if c.idle > 0 {
        (include_bytes!("../icons/tray-coffee.png"), String::new())
    } else {
        (include_bytes!("../icons/tray-moon-stars.png"), String::new())
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
        let _ = tray.set_tooltip(Some(format!("{} need you · {} working · {} idle{quiet}", c.needs, c.busy, c.idle)));
    }
    if let Some(window) = app.get_webview_window("main") {
        let needs = snap.model.counts.needs as i64;
        let _ = window.set_badge_count(if needs > 0 { Some(needs) } else { None });
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// A short system sound. macOS only; elsewhere the notification is the only signal.
fn play_sound() {
    #[cfg(target_os = "macos")]
    let _ = spawn_reaped(Command::new("afplay").arg("/System/Library/Sounds/Glass.aiff"));
}

/// Blink the menu bar icon a few times, then put back the icon of the current state.
fn flash_tray(app: AppHandle) {
    std::thread::spawn(move || {
        let hand = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-hand-palm.png"));
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
                    let model = build(&raw);
                    let now = now_ms();
                    if let Ok(mut a) = app.state::<AlertState>().0.lock() { outcome = a.on_read(&model, now); }
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
        for alert in &outcome.alerts { notify(&app, &alert.title, &alert.body); }
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

fn main() {
    let (wake_tx, wake_rx) = channel::<()>();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .manage(Shared::default())
        .manage(Wake(Mutex::new(wake_tx)))
        .manage(AlertState(Mutex::new(Alerts::new(true))))
        .manage(DndMenu(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![snapshot, refresh, open_session, set_dnd])
        .setup(move |app| {
            let handle = app.handle().clone();
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
                .show_menu_on_left_click(true)
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
        .on_window_event(|window, event| {
            // Closing the window hides it: the app keeps watching and the Dock icon opens it again.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
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
