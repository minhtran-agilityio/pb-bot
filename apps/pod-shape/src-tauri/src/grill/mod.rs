//! Build step 3, Grill (HANDOFF §6 step 3; specs design, advisor-reviewed). The Tauri commands live
//! here, apart from the artifact commands in lib.rs.

pub mod adr;
pub mod brief;
pub mod engine;
#[cfg(test)]
mod engine_tests;
pub mod flags;
pub mod p2g;
pub mod proc;
pub mod store;
pub mod stream;

use engine::Engine;
use store::{SessionDetail, SessionSummary};

/// Event the UI listens to: `{ sessionId }` changed (a message, a state, a run).
pub const GRILL_CHANGED: &str = "grill-changed";

pub struct Grill(pub Engine);

fn lock(db: &std::sync::Mutex<rusqlite::Connection>) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
    db.lock().unwrap_or_else(|p| p.into_inner())
}

/// Every command that starts a turn returns at once; the turn runs on its own thread.
#[tauri::command]
pub async fn grill_start(g: tauri::State<'_, Grill>, version_id: i64) -> Result<i64, String> {
    let e = g.0.clone();
    tauri::async_runtime::spawn_blocking(move || e.start(version_id)).await.map_err(|x| x.to_string())?
}

#[tauri::command]
pub async fn grill_sessions(g: tauri::State<'_, Grill>, artifact_id: i64) -> Result<Vec<SessionSummary>, String> {
    let db = g.0.db.clone();
    tauri::async_runtime::spawn_blocking(move || store::list_sessions(&lock(&db), artifact_id).map_err(|x| x.to_string()))
        .await
        .map_err(|x| x.to_string())?
}

/// The newest session of each artifact of a project (the card buttons).
#[tauri::command]
pub async fn grill_latest(g: tauri::State<'_, Grill>, project_id: i64) -> Result<Vec<SessionSummary>, String> {
    let db = g.0.db.clone();
    tauri::async_runtime::spawn_blocking(move || store::latest_sessions(&lock(&db), project_id).map_err(|x| x.to_string()))
        .await
        .map_err(|x| x.to_string())?
}

#[tauri::command]
pub async fn grill_detail(g: tauri::State<'_, Grill>, session_id: i64) -> Result<SessionDetail, String> {
    let db = g.0.db.clone();
    tauri::async_runtime::spawn_blocking(move || store::detail(&lock(&db), session_id)).await.map_err(|x| x.to_string())?
}

/// One answer to one open question; `adr` only after the owner confirmed the ADR dialog.
/// Returns true when the block was complete and the turn was sent.
#[tauri::command]
pub async fn grill_answer(g: tauri::State<'_, Grill>, session_id: i64, question: String, answer: String, adr: bool) -> Result<bool, String> {
    let e = g.0.clone();
    tauri::async_runtime::spawn_blocking(move || e.answer(session_id, &question, &answer, adr)).await.map_err(|x| x.to_string())?
}

#[tauri::command]
pub async fn grill_send(g: tauri::State<'_, Grill>, session_id: i64, text: String) -> Result<(), String> {
    let e = g.0.clone();
    tauri::async_runtime::spawn_blocking(move || e.send_text(session_id, &text)).await.map_err(|x| x.to_string())?
}

#[tauri::command]
pub async fn grill_confirm_brief(g: tauri::State<'_, Grill>, session_id: i64) -> Result<(), String> {
    let e = g.0.clone();
    tauri::async_runtime::spawn_blocking(move || e.confirm_brief(session_id)).await.map_err(|x| x.to_string())?
}

#[tauri::command]
pub fn grill_stop(g: tauri::State<'_, Grill>, session_id: i64) -> Result<(), String> {
    g.0.stop(session_id)
}

#[tauri::command]
pub async fn grill_resume(g: tauri::State<'_, Grill>, session_id: i64) -> Result<(), String> {
    let e = g.0.clone();
    tauri::async_runtime::spawn_blocking(move || e.resume(session_id)).await.map_err(|x| x.to_string())?
}

/// The owner read the prepared ADR: `publish` pushes `adr/<slug>` and opens the pull request; otherwise
/// the ADR is discarded. Returns the note shown in the chat.
#[tauri::command]
pub async fn grill_adr_decide(g: tauri::State<'_, Grill>, session_id: i64, publish: bool) -> Result<String, String> {
    let e = g.0.clone();
    tauri::async_runtime::spawn_blocking(move || e.adr_decide(session_id, publish)).await.map_err(|x| x.to_string())?
}
