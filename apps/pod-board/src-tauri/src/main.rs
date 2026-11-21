// Pod Board: a small desktop window with every Pod Bay session by state (needs you, busy, idle).
// It only observes. It runs the read-only commands `podbay-dev ls --all`, `checkouts --all`, `machines` and
// `queue ls --all`; it never calls `podbay-dev detail` and never sends anything anywhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod model;

use model::{build, key, newly_needs, Model, Raw};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;

const POLL_PAUSE: Duration = Duration::from_secs(3); // one `ls --all` takes about 1.3 s
const SLOW_EVERY: u32 = 10; // checkouts, machines and the queue change slowly
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, Serialize)]
struct Snapshot {
    ok: bool,
    error: Option<String>,
    read_at: Option<u64>,
    #[serde(flatten)]
    model: Model,
}

#[derive(Default)]
struct Shared(Mutex<Snapshot>);

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
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = out.read_to_string(&mut text);
        text
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
    let text = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("podbay-dev {} failed", args[0]));
    }
    serde_json::from_str(&text).map_err(|_| format!("podbay-dev {} did not return JSON", args[0]))
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
        let _ = tray.set_tooltip(Some(format!("{} need you · {} working · {} idle", c.needs, c.busy, c.idle)));
    }
    if let Some(window) = app.get_webview_window("main") {
        let needs = snap.model.counts.needs as i64;
        let _ = window.set_badge_count(if needs > 0 { Some(needs) } else { None });
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn poll_loop(app: AppHandle) {
    let cmd = find_podbay();
    let mut raw = Raw::default();
    let mut before: HashMap<String, &'static str> = HashMap::new();
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

        let snap = {
            let state = app.state::<Shared>();
            let mut held = match state.0.lock() { Ok(g) => g, Err(p) => p.into_inner() };
            match error {
                // A failed read keeps the last sessions and says why.
                Some(e) => { held.ok = false; held.error = Some(e); }
                None => {
                    let model = build(&raw);
                    for s in newly_needs(&before, &model) {
                        let what = if s.what.is_empty() { format!("{}/{} · {}", s.project, s.ws, s.machine) } else { s.what.clone() };
                        notify(&app, &format!("{} needs you", s.title), &what);
                    }
                    before = model.sessions.iter().map(|s| (key(s), s.state)).collect();
                    if before.is_empty() {
                        // Keep "first read" apart from "no session": a marker stops a later first session from being silent.
                        before.insert(String::new(), "idle");
                    }
                    *held = Snapshot { ok: true, error: None, read_at: Some(now_ms()), model };
                }
            }
            held.clone()
        };
        publish(&app, &snap);
        std::thread::sleep(POLL_PAUSE);
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
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .manage(Shared::default())
        .invoke_handler(tauri::generate_handler![snapshot])
        .setup(|app| {
            let handle = app.handle().clone();
            let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
            let open = MenuItem::with_id(app, "open", "Open Pod Board", true, None::<&str>)?;
            let autostart = CheckMenuItem::with_id(app, "autostart", "Start at login", true, autostart_on, None::<&str>)?;
            let on_top = CheckMenuItem::with_id(app, "on_top", "Always on top", true, false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &autostart, &on_top, &PredefinedMenuItem::separator(app)?, &quit])?;
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
            std::thread::spawn(move || poll_loop(handle));
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
