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

/// Saves a project after the dialog's checks passed. The folder check runs again here,
/// so the database never holds a folder that does not match its repo.
#[tauri::command]
async fn add_project(db: tauri::State<'_, Db>, repo: String, path: String) -> Result<Project, String> {
    let repo = checks::normalize_repo(&repo).ok_or("Type the GitHub repo as owner/name.")?;
    let (r, p) = (repo.clone(), path.clone());
    let folder = tauri::async_runtime::spawn_blocking(move || checks::check_folder(&r, &p))
        .await
        .map_err(err)?;
    if !folder.ok {
        return Err(folder.detail);
    }
    let abs = checks::expand_home(&path);
    let abs = abs.canonicalize().unwrap_or(abs).to_string_lossy().into_owned();
    let name = repo.rsplit('/').next().unwrap_or(&repo).to_string();
    let conn = db.0.lock().map_err(err)?;
    if let Some(existing) = db::find_by_repo(&conn, &repo).map_err(err)? {
        return Err(format!("{} is already a project.", existing.repo));
    }
    db::insert_project(&conn, &repo, &name, &abs).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    checks::fix_path_for_gui_app();
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
        .invoke_handler(tauri::generate_handler![list_projects, run_check, add_project])
        .run(tauri::generate_context!())
        .expect("error while running Shape Board");
}
