mod checks;
mod db;

use checks::CheckResult;
use db::Project;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

struct Db(Mutex<Connection>);

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn list_projects(db: tauri::State<'_, Db>) -> Result<Vec<Project>, String> {
    let conn = db.0.lock().map_err(err)?;
    db::list_projects(&conn).map_err(err)
}

/// One check of the Add project dialog: `gh`, `claude` or `folder`.
/// Async so the three run in parallel off the main thread.
#[tauri::command]
async fn run_check(kind: String, repo: String, path: String) -> Result<CheckResult, String> {
    tauri::async_runtime::spawn_blocking(move || match kind.as_str() {
        "gh" => Ok(checks::check_gh()),
        "claude" => Ok(checks::check_claude()),
        "folder" => Ok(checks::check_folder(&repo, &path)),
        other => Err(format!("unknown check: {other}")),
    })
    .await
    .map_err(err)?
}

/// Cancel button of the Add project dialog: ends every running check and any save in progress.
/// Running `run_check` calls return a failed result with the detail "Cancelled.".
#[tauri::command]
fn cancel_checks() {
    checks::cancel_checks();
}

/// `owner/name` for any accepted input form (`owner/name`, `github.com/o/n`, `https://github.com/o/n.git`).
#[tauri::command]
fn normalize_repo(input: String) -> Result<String, String> {
    checks::normalize_repo(&input).ok_or_else(|| checks::REPO_FORMAT_ERROR.to_string())
}

/// Saves a project after the dialog's checks passed. The folder check runs again here,
/// so the database never holds a folder that does not match its repo. The repo is stored in
/// the case of the folder's origin remote, not the case the owner typed.
#[tauri::command]
async fn add_project(db: tauri::State<'_, Db>, repo: String, path: String) -> Result<Project, String> {
    // A cancel after this point must stop the save (see the check right before the insert).
    let epoch = checks::cancel_epoch();
    let typed = checks::normalize_repo(&repo).ok_or(checks::REPO_FORMAT_ERROR)?;
    let dir = checks::resolve_folder(&path)?;
    let (r, p) = (typed, path.clone());
    let folder = tauri::async_runtime::spawn_blocking(move || checks::check_folder(&r, &p))
        .await
        .map_err(err)?;
    if !folder.ok {
        return Err(folder.detail);
    }
    // On success `detail` is the origin's owner/name.
    let repo = folder.detail;
    let abs = dir.canonicalize().unwrap_or(dir).to_string_lossy().into_owned();
    let name = repo.rsplit('/').next().unwrap_or(&repo).to_string();
    let conn = db.0.lock().map_err(err)?;
    if db::find_by_repo(&conn, &repo).map_err(err)?.is_some() {
        return Err("This repo is already added.".to_string());
    }
    checks::ensure_not_cancelled(epoch)?;
    db::insert_project(&conn, &repo, &name, &abs).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Looks up the shell PATH on a background thread; the checks wait for it, the window does not.
    checks::start_path_fix();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("shape-board.db"))?;
            app.manage(Db(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_projects, run_check, cancel_checks, add_project, normalize_repo])
        .run(tauri::generate_context!())
        .expect("error while running Shape Board");
}
