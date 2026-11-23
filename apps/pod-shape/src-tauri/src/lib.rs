mod artifacts;
mod checks;
mod db;
mod nav;

use artifacts::paths::HookPaths;
use artifacts::store::{self, ArtifactSummary, FieldErrors, InboxItem, ManualResult, ManualTarget, VersionInfo};
use artifacts::sync::SyncReport;
use artifacts::watch::HookStatus;
use checks::CheckResult;
use db::Project;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::webview::NewWindowResponse;
use tauri::{Emitter, Manager, WebviewWindowBuilder};

struct Db(Arc<Mutex<Connection>>);
struct Hook(HookPaths);

/// Event the UI listens to: the hook brought something (or the folder or log appeared or went away).
const ARTIFACTS_CHANGED: &str = "artifacts-changed";

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

// ---------- step 2: artifacts ----------

#[tauri::command]
fn hook_status(hook: tauri::State<'_, Hook>) -> HookStatus {
    artifacts::watch::status(&hook.0, artifacts::sync::bad_lines(&hook.0))
}

/// Runs one sync now (also what the watcher runs on each change).
#[tauri::command]
async fn rescan_hook(db: tauri::State<'_, Db>, hook: tauri::State<'_, Hook>) -> Result<SyncReport, String> {
    let (db, paths) = (db.0.clone(), hook.0.clone());
    tauri::async_runtime::spawn_blocking(move || artifacts::sync::sync(&db, &paths)).await.map_err(err)
}

#[tauri::command]
fn list_inbox(db: tauri::State<'_, Db>) -> Result<Vec<InboxItem>, String> {
    store::list_inbox(&*db.0.lock().map_err(err)?).map_err(err)
}

#[tauri::command]
fn add_inbox_artifact(db: tauri::State<'_, Db>, artifact_id: i64, project_id: i64) -> Result<(), String> {
    store::add_to_project(&*db.0.lock().map_err(err)?, artifact_id, project_id)
}

#[tauri::command]
fn ignore_artifact(db: tauri::State<'_, Db>, artifact_id: i64) -> Result<(), String> {
    store::ignore(&*db.0.lock().map_err(err)?, artifact_id).map_err(err)
}

#[tauri::command]
fn list_artifacts(db: tauri::State<'_, Db>, project_id: i64) -> Result<Vec<ArtifactSummary>, String> {
    store::list_artifacts(&*db.0.lock().map_err(err)?, project_id).map_err(err)
}

/// Artifact count per project id, for the project cards.
#[tauri::command]
fn artifact_counts(db: tauri::State<'_, Db>) -> Result<HashMap<i64, i64>, String> {
    Ok(store::artifact_counts(&*db.0.lock().map_err(err)?).map_err(err)?.into_iter().collect())
}

#[tauri::command]
fn list_versions(db: tauri::State<'_, Db>, artifact_id: i64) -> Result<Vec<VersionInfo>, String> {
    store::list_versions(&*db.0.lock().map_err(err)?, artifact_id).map_err(err)
}

/// Opening the preview clears the "Updated" mark (owner decision 2026-11-23).
#[tauri::command]
fn mark_artifact_seen(db: tauri::State<'_, Db>, artifact_id: i64) -> Result<(), String> {
    store::mark_seen(&*db.0.lock().map_err(err)?, artifact_id).map_err(err)
}

fn opt(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Where the Add artifact dialog would save: an existing artifact (its next version) or a new one.
#[tauri::command]
fn find_manual_target(
    db: tauri::State<'_, Db>,
    project_id: i64,
    link: Option<String>,
    name: String,
) -> Result<Result<ManualTarget, FieldErrors>, String> {
    let link = opt(link);
    Ok(store::find_manual_target(&*db.0.lock().map_err(err)?, project_id, link.as_deref(), name.trim()))
}

/// Saves pasted or dropped HTML (`source` is `paste` or `drop`). Field errors come back as data.
#[tauri::command]
fn import_manual(
    db: tauri::State<'_, Db>,
    project_id: i64,
    link: Option<String>,
    name: String,
    html: String,
    source: String,
) -> Result<Result<ManualResult, FieldErrors>, String> {
    let link = opt(link);
    let mut conn = db.0.lock().map_err(err)?;
    Ok(store::import_manual(&mut conn, project_id, link.as_deref(), name.trim(), &html, &source))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Looks up the shell PATH on a background thread; the checks wait for it, the window does not.
    checks::start_path_fix();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // The sandboxed preview frame loads preview://localhost/v/<versionId> (artifacts/preview.rs).
        // Asynchronous, so the database lock is never taken on the UI thread.
        .register_asynchronous_uri_scheme_protocol("preview", |ctx, req, responder| {
            let label = ctx.webview_label().to_string();
            let db = ctx.app_handle().try_state::<Db>().map(|d| d.0.clone());
            tauri::async_runtime::spawn_blocking(move || {
                let uri = req.uri();
                let r = match db {
                    Some(db) => {
                        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                        let pr = artifacts::preview::PreviewRequest {
                            method: req.method().as_str(),
                            host: uri.host(),
                            path: uri.path(),
                            query: uri.query(),
                            webview_label: &label,
                        };
                        artifacts::preview::respond(&conn, &pr)
                    }
                    None => artifacts::preview::not_found(),
                };
                let mut b = tauri::http::Response::builder().status(r.status);
                for (k, v) in r.headers {
                    b = b.header(k, v);
                }
                responder.respond(b.body(r.body).unwrap_or_default());
            });
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = Arc::new(Mutex::new(db::open(&dir.join("shape-board.db"))?));
            let paths = HookPaths::for_home(&app.path().home_dir()?);
            app.manage(Db(conn.clone()));
            app.manage(Hook(paths.clone()));
            // The main window is built here, not from the config, so it gets the navigation guard
            // (nav.rs): untrusted HTML never becomes a main-frame document. New windows and downloads
            // are denied.
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .ok_or("the main window is missing from tauri.conf.json")?;
            let dev_url = if cfg!(debug_assertions) { app.config().build.dev_url.clone() } else { None };
            WebviewWindowBuilder::from_config(app.handle(), &config)?
                .on_navigation(move |url| {
                    let ok = nav::allowed(url, dev_url.as_ref());
                    if !ok {
                        eprintln!("shape-board: blocked navigation to {}", url.scheme());
                    }
                    ok
                })
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .on_download(|_, _| false)
                .build()?;
            // Full sync at start, then on every change; the window does not wait for it.
            let handle = app.handle().clone();
            artifacts::watch::start(conn, paths, move |report| {
                let _ = handle.emit(ARTIFACTS_CHANGED, report);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            run_check,
            cancel_checks,
            add_project,
            normalize_repo,
            hook_status,
            rescan_hook,
            list_inbox,
            add_inbox_artifact,
            ignore_artifact,
            list_artifacts,
            artifact_counts,
            list_versions,
            mark_artifact_seen,
            find_manual_target,
            import_manual
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shape Board");
}
