//! SQLite side of the grill: sessions, their history, briefs, features and runs (migration 0003).

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

pub const STATES: [&str; 6] = ["running", "waiting_owner", "brief_ready", "confirmed", "stopped", "failed"];

/// Shown after an app restart for a session that was running (design §2.5).
pub const NOTE_APP_CLOSED: &str = "The app closed while Claude was running. Press Resume to continue.";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: i64,
    pub artifact_id: i64,
    pub artifact_version_id: i64,
    pub version_n: i64,
    pub n: i64,
    pub state: String,
    /// The confirmed feature's name, else the latest brief's title, else null.
    pub feature_name: Option<String>,
    pub messages: i64,
    pub answers: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub turn: i64,
    pub role: String,
    pub text: String,
    pub questions: Option<Value>,
    pub answered: bool,
    pub interrupted: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub id: i64,
    pub step: String,
    pub command: String,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub ms: Option<i64>,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BriefRow {
    pub id: i64,
    pub n: i64,
    pub path: String,
    pub content: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

/// The question block that waits for the owner, with the answers given so far.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenBlock {
    pub message_id: i64,
    pub questions: Value,
    pub drafts: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub error: Option<String>,
    pub has_pending: bool,
    pub adr_pending: bool,
    pub project_path: String,
    pub messages: Vec<Message>,
    pub runs: Vec<RunRow>,
    pub brief: Option<BriefRow>,
    pub feature: Option<Feature>,
    pub open_block: Option<OpenBlock>,
}

/// What a turn needs from the database.
#[derive(Debug, Clone)]
pub struct TurnContext {
    pub uuid: String,
    pub claude_started: bool,
    pub state: String,
    pub project_path: String,
    pub html: String,
    pub turns: i64,
    pub adr_pending: bool,
    pub app_note: Option<String>,
}

const SUMMARY_SQL: &str = "SELECT s.id, s.artifact_id, s.artifact_version_id, v.n, s.n, s.state,
    COALESCE((SELECT f.name FROM features f WHERE f.grill_session_id = s.id),
             (SELECT b.content FROM briefs b WHERE b.grill_session_id = s.id ORDER BY b.n DESC LIMIT 1)),
    (SELECT COUNT(*) FROM grill_messages m WHERE m.session_id = s.id AND m.role IN ('owner', 'claude')),
    (SELECT COUNT(*) FROM grill_messages m WHERE m.session_id = s.id AND m.role = 'owner' AND m.turn > 0),
    s.created_at, s.updated_at
  FROM grill_sessions s JOIN artifact_versions v ON v.id = s.artifact_version_id";

fn row_summary(r: &rusqlite::Row) -> rusqlite::Result<SessionSummary> {
    let name: Option<String> = r.get(6)?;
    Ok(SessionSummary {
        id: r.get(0)?,
        artifact_id: r.get(1)?,
        artifact_version_id: r.get(2)?,
        version_n: r.get(3)?,
        n: r.get(4)?,
        state: r.get(5)?,
        // A brief's content is turned into its title; a feature name is already a name.
        feature_name: name.map(|n| if n.contains('\n') || n.starts_with('#') { brief_title(&n) } else { n }),
        messages: r.get(7)?,
        answers: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

/// The first `# ` heading, without a leading "Brief:" (pd-shape's template starts "# Brief: <name>").
pub fn brief_title(content: &str) -> String {
    content
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().trim_start_matches("Brief:").trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled feature".to_string())
}

/// Creates a session on one artifact version. The caller then starts its first turn.
pub fn create_session(conn: &Connection, version_id: i64, uuid: &str) -> Result<i64, String> {
    let artifact_id: i64 = conn
        .query_row("SELECT artifact_id FROM artifact_versions WHERE id = ?1", [version_id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("This version no longer exists.")?;
    let project: Option<i64> = conn
        .query_row("SELECT project_id FROM artifacts WHERE id = ?1", [artifact_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if project.is_none() {
        return Err("Add this artifact to a project first.".into());
    }
    let open: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM grill_sessions WHERE artifact_id = ?1 AND state != 'confirmed'",
            [artifact_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if open > 0 {
        return Err("Finish or confirm the open session first.".into());
    }
    let n: i64 = conn
        .query_row("SELECT COALESCE(MAX(n), 0) + 1 FROM grill_sessions WHERE artifact_id = ?1", [artifact_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO grill_sessions (artifact_version_id, artifact_id, claude_session_id, n, state) VALUES (?1, ?2, ?3, ?4, 'waiting_owner')",
        params![version_id, artifact_id, uuid, n],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn list_sessions(conn: &Connection, artifact_id: i64) -> rusqlite::Result<Vec<SessionSummary>> {
    let mut st = conn.prepare(&format!("{SUMMARY_SQL} WHERE s.artifact_id = ?1 ORDER BY s.n DESC"))?;
    let rows = st.query_map([artifact_id], row_summary)?;
    rows.collect()
}

pub fn summary(conn: &Connection, id: i64) -> rusqlite::Result<Option<SessionSummary>> {
    conn.query_row(&format!("{SUMMARY_SQL} WHERE s.id = ?1"), [id], row_summary).optional()
}

/// The newest session per artifact of a project, for the "Start grill" / "Open grill" buttons.
pub fn latest_sessions(conn: &Connection, project_id: i64) -> rusqlite::Result<Vec<SessionSummary>> {
    let mut st = conn.prepare(&format!(
        "{SUMMARY_SQL} JOIN artifacts a ON a.id = s.artifact_id WHERE a.project_id = ?1
         AND s.n = (SELECT MAX(n) FROM grill_sessions x WHERE x.artifact_id = s.artifact_id)"
    ))?;
    let rows = st.query_map([project_id], row_summary)?;
    rows.collect()
}

pub fn messages(conn: &Connection, id: i64) -> rusqlite::Result<Vec<Message>> {
    let mut st = conn.prepare(
        "SELECT id, turn, role, text, questions, answered, interrupted, created_at FROM grill_messages
         WHERE session_id = ?1 ORDER BY id",
    )?;
    let rows = st.query_map([id], |r| {
        let q: Option<String> = r.get(4)?;
        Ok(Message {
            id: r.get(0)?,
            turn: r.get(1)?,
            role: r.get(2)?,
            text: r.get(3)?,
            questions: q.and_then(|q| serde_json::from_str(&q).ok()),
            answered: r.get::<_, i64>(5)? != 0,
            interrupted: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn runs(conn: &Connection, id: i64) -> rusqlite::Result<Vec<RunRow>> {
    let mut st = conn.prepare(
        "SELECT id, step, command, exit_code, error, ms, ended_at IS NULL FROM runs WHERE grill_session_id = ?1 ORDER BY id",
    )?;
    let rows = st.query_map([id], |r| {
        Ok(RunRow {
            id: r.get(0)?,
            step: r.get(1)?,
            command: r.get(2)?,
            exit_code: r.get(3)?,
            error: r.get(4)?,
            ms: r.get(5)?,
            running: r.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn latest_brief(conn: &Connection, id: i64) -> rusqlite::Result<Option<BriefRow>> {
    conn.query_row(
        "SELECT id, n, path, content, confirmed_at IS NOT NULL FROM briefs WHERE grill_session_id = ?1 ORDER BY n DESC LIMIT 1",
        [id],
        |r| Ok(BriefRow { id: r.get(0)?, n: r.get(1)?, path: r.get(2)?, content: r.get(3)?, confirmed: r.get(4)? }),
    )
    .optional()
}

pub fn feature(conn: &Connection, id: i64) -> rusqlite::Result<Option<Feature>> {
    conn.query_row("SELECT id, name, slug FROM features WHERE grill_session_id = ?1", [id], |r| {
        Ok(Feature { id: r.get(0)?, name: r.get(1)?, slug: r.get(2)? })
    })
    .optional()
}

/// The newest claude message with questions that are not answered yet, if no owner turn came after it.
pub fn open_block(conn: &Connection, id: i64) -> rusqlite::Result<Option<OpenBlock>> {
    let row: Option<(i64, String, i64)> = conn
        .query_row(
            "SELECT id, questions, answered FROM grill_messages WHERE session_id = ?1 AND role = 'claude' AND questions IS NOT NULL
             ORDER BY id DESC LIMIT 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((mid, q, answered)) = row else { return Ok(None) };
    if answered != 0 {
        return Ok(None);
    }
    let drafts: Option<String> = conn.query_row("SELECT draft_answers FROM grill_sessions WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(Some(OpenBlock {
        message_id: mid,
        questions: serde_json::from_str(&q).unwrap_or(Value::Null),
        drafts: drafts.and_then(|d| serde_json::from_str(&d).ok()).unwrap_or_else(|| serde_json::json!({})),
    }))
}

pub fn detail(conn: &Connection, id: i64) -> Result<SessionDetail, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let session = summary(conn, id).map_err(e)?.ok_or("This grill session no longer exists.")?;
    let (error, pending, adr, path): (Option<String>, Option<String>, i64, String) = conn
        .query_row(
            "SELECT s.error, s.pending_message, s.adr_pending, p.path FROM grill_sessions s
             JOIN artifacts a ON a.id = s.artifact_id JOIN projects p ON p.id = a.project_id WHERE s.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(e)?;
    Ok(SessionDetail {
        session,
        error,
        has_pending: pending.is_some(),
        adr_pending: adr != 0,
        project_path: path,
        messages: messages(conn, id).map_err(e)?,
        runs: runs(conn, id).map_err(e)?,
        brief: latest_brief(conn, id).map_err(e)?,
        feature: feature(conn, id).map_err(e)?,
        open_block: open_block(conn, id).map_err(e)?,
    })
}

pub fn turn_context(conn: &Connection, id: i64) -> Result<TurnContext, String> {
    conn.query_row(
        "SELECT s.claude_session_id, s.claude_started, s.state, p.path, v.html, s.turns, s.adr_pending, s.app_note
         FROM grill_sessions s JOIN artifact_versions v ON v.id = s.artifact_version_id
         JOIN artifacts a ON a.id = s.artifact_id JOIN projects p ON p.id = a.project_id WHERE s.id = ?1",
        [id],
        |r| {
            Ok(TurnContext {
                uuid: r.get(0)?,
                claude_started: r.get::<_, i64>(1)? != 0,
                state: r.get(2)?,
                project_path: r.get(3)?,
                html: r.get(4)?,
                turns: r.get(5)?,
                adr_pending: r.get::<_, i64>(6)? != 0,
                app_note: r.get(7)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "This grill session no longer exists.".to_string())
}

pub fn set_state(conn: &Connection, id: i64, state: &str) -> rusqlite::Result<()> {
    debug_assert!(STATES.contains(&state));
    let ended = if state == "confirmed" { "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')" } else { "NULL" };
    conn.execute(
        &format!("UPDATE grill_sessions SET state = ?2, ended_at = {ended}, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1"),
        params![id, state],
    )?;
    Ok(())
}

pub fn add_message(conn: &Connection, id: i64, turn: i64, role: &str, text: &str, raw: Option<&str>) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO grill_messages (session_id, turn, role, text, raw) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, turn, role, text, raw],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Marks the turn's claude and tool rows as interrupted (Stop, timeout, app closed).
pub fn mark_interrupted(conn: &Connection, id: i64, turn: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE grill_messages SET interrupted = 1 WHERE session_id = ?1 AND turn = ?2 AND role IN ('claude', 'tool')",
        params![id, turn],
    )?;
    Ok(())
}

/// Every session that was `running` when the app stopped becomes `stopped`, with a note
/// (call after the orphan sweep). Returns how many.
pub fn recover_running(conn: &Connection) -> rusqlite::Result<usize> {
    let ids: Vec<(i64, i64)> = {
        let mut st = conn.prepare("SELECT id, turns FROM grill_sessions WHERE state = 'running'")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (id, turn) in &ids {
        mark_interrupted(conn, *id, *turn)?;
        add_message(conn, *id, *turn, "note", NOTE_APP_CLOSED, None)?;
        set_state(conn, *id, "stopped")?;
    }
    conn.execute(
        "UPDATE runs SET ended_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), error = COALESCE(error, 'The app closed while this ran.')
         WHERE ended_at IS NULL",
        [],
    )?;
    Ok(ids.len())
}

/// Runs that never ended (the app crashed): (pgid, stored start time).
pub fn live_runs(conn: &Connection) -> rusqlite::Result<Vec<(i32, Option<String>)>> {
    let mut st = conn.prepare("SELECT pgid, proc_start FROM runs WHERE ended_at IS NULL AND pgid IS NOT NULL")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

pub fn start_run(conn: &Connection, id: i64, step: &str, command: &str, args: &[String]) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO runs (grill_session_id, step, command, args) VALUES (?1, ?2, ?3, ?4)",
        params![id, step, command, serde_json::to_string(args).unwrap_or_default()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_run_process(conn: &Connection, run: i64, pgid: i32, proc_start: Option<&str>) -> rusqlite::Result<()> {
    conn.execute("UPDATE runs SET pgid = ?2, proc_start = ?3 WHERE id = ?1", params![run, pgid, proc_start])?;
    Ok(())
}

pub fn end_run(conn: &Connection, run: i64, exit_code: Option<i32>, error: Option<&str>, cost: Option<f64>, ms: u128) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE runs SET exit_code = ?2, error = ?3, cost_usd = ?4, ms = ?5, ended_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
        params![run, exit_code, error, cost, ms as i64],
    )?;
    Ok(())
}

/// Adds a brief version when its text differs from the latest one. Returns whether one was added.
pub fn add_brief_version(conn: &Connection, id: i64, path: &str, content: &str, sha: &str) -> rusqlite::Result<bool> {
    let latest: Option<String> =
        conn.query_row("SELECT sha256 FROM briefs WHERE grill_session_id = ?1 ORDER BY n DESC LIMIT 1", [id], |r| r.get(0)).optional()?;
    if latest.as_deref() == Some(sha) {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO briefs (grill_session_id, n, path, content, sha256)
         VALUES (?1, (SELECT COALESCE(MAX(n), 0) + 1 FROM briefs WHERE grill_session_id = ?1), ?2, ?3, ?4)",
        params![id, path, content, sha],
    )?;
    Ok(true)
}

/// The owner's confirmation: the latest brief is confirmed and the feature exists (1 grill = 1 feature).
pub fn confirm_brief(conn: &Connection, id: i64) -> Result<Feature, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let b = latest_brief(conn, id).map_err(e)?.ok_or("There is no brief to confirm yet.")?;
    conn.execute("UPDATE briefs SET confirmed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1", [b.id]).map_err(e)?;
    if let Some(f) = feature(conn, id).map_err(e)? {
        return Ok(f);
    }
    let name = brief_title(&b.content);
    // specs/<slug>/brief.md -> <slug>
    let slug = b.path.trim_start_matches("specs/").split('/').next().unwrap_or("feature").to_string();
    conn.execute("INSERT INTO features (grill_session_id, name, slug) VALUES (?1, ?2, ?3)", params![id, name, slug]).map_err(e)?;
    feature(conn, id).map_err(e)?.ok_or_else(|| "The feature was not saved.".into())
}

pub fn set_draft(conn: &Connection, id: i64, drafts: Option<&Value>) -> rusqlite::Result<()> {
    conn.execute("UPDATE grill_sessions SET draft_answers = ?2 WHERE id = ?1", params![id, drafts.map(|d| d.to_string())])?;
    Ok(())
}

pub fn mark_answered(conn: &Connection, message_id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE grill_messages SET answered = 1 WHERE id = ?1", [message_id])?;
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A database with one project, one artifact and one version. Returns (dir, conn, version id).
    pub fn seeded() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        crate::db::insert_project(&conn, "a/b", "b", &repo.to_string_lossy()).unwrap();
        conn.execute("INSERT INTO artifacts (project_id, name, source, state) VALUES (1, 'proto', 'manual', 'added')", []).unwrap();
        conn.execute("INSERT INTO artifact_versions (artifact_id, n, html, sha256, source) VALUES (1, 1, '<p>hi</p>', 'h', 'paste')", []).unwrap();
        (dir, conn, 1)
    }

    #[test]
    fn a_second_open_session_on_the_same_artifact_is_refused() {
        let (_d, conn, v) = seeded();
        let id = create_session(&conn, v, "u1").unwrap();
        assert_eq!(summary(&conn, id).unwrap().unwrap().n, 1);
        assert_eq!(create_session(&conn, v, "u2").unwrap_err(), "Finish or confirm the open session first.");
        set_state(&conn, id, "confirmed").unwrap();
        let id2 = create_session(&conn, v, "u2").unwrap();
        assert_eq!(summary(&conn, id2).unwrap().unwrap().n, 2);
        assert_eq!(list_sessions(&conn, 1).unwrap().iter().map(|s| s.n).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn recovery_turns_running_into_stopped_with_a_note_and_keeps_the_pending_message() {
        let (_d, conn, v) = seeded();
        let id = create_session(&conn, v, "u1").unwrap();
        conn.execute("UPDATE grill_sessions SET state = 'running', turns = 2, pending_message = '{\"answers\":{}}' WHERE id = ?1", [id]).unwrap();
        add_message(&conn, id, 2, "claude", "half a sentence", None).unwrap();
        let run = start_run(&conn, id, "grill", "claude -p", &[]).unwrap();
        assert_eq!(recover_running(&conn).unwrap(), 1);
        let d = detail(&conn, id).unwrap();
        assert_eq!(d.session.state, "stopped");
        assert!(d.has_pending);
        assert!(d.messages.iter().any(|m| m.role == "claude" && m.interrupted));
        assert_eq!(d.messages.last().unwrap().text, NOTE_APP_CLOSED);
        let ended: Option<String> = conn.query_row("SELECT ended_at FROM runs WHERE id = ?1", [run], |r| r.get(0)).unwrap();
        assert!(ended.is_some());
    }

    #[test]
    fn briefs_are_versioned_by_content_and_confirming_makes_one_feature() {
        let (_d, conn, v) = seeded();
        let id = create_session(&conn, v, "u1").unwrap();
        assert!(add_brief_version(&conn, id, "specs/phone-sign-up/brief.md", "# Brief: Phone sign-up\n", "a").unwrap());
        assert!(!add_brief_version(&conn, id, "specs/phone-sign-up/brief.md", "# Brief: Phone sign-up\n", "a").unwrap());
        assert!(add_brief_version(&conn, id, "specs/phone-sign-up/brief.md", "# Brief: Phone sign-up\nmore", "b").unwrap());
        assert_eq!(latest_brief(&conn, id).unwrap().unwrap().n, 2);
        assert_eq!(summary(&conn, id).unwrap().unwrap().feature_name.as_deref(), Some("Phone sign-up"));
        let f = confirm_brief(&conn, id).unwrap();
        assert_eq!((f.name.as_str(), f.slug.as_str()), ("Phone sign-up", "phone-sign-up"));
        assert!(latest_brief(&conn, id).unwrap().unwrap().confirmed);
        // A later version is unconfirmed; confirming again keeps the one feature.
        add_brief_version(&conn, id, "specs/phone-sign-up/brief.md", "# Brief: Phone sign-up\nADRs: 0007", "c").unwrap();
        assert!(!latest_brief(&conn, id).unwrap().unwrap().confirmed);
        assert_eq!(confirm_brief(&conn, id).unwrap().id, f.id);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM features", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn the_open_block_is_the_newest_unanswered_question_message() {
        let (_d, conn, v) = seeded();
        let id = create_session(&conn, v, "u1").unwrap();
        assert!(open_block(&conn, id).unwrap().is_none());
        let m = add_message(&conn, id, 1, "claude", "Q", None).unwrap();
        conn.execute("UPDATE grill_messages SET questions = '{\"questions\":[]}' WHERE id = ?1", [m]).unwrap();
        set_draft(&conn, id, Some(&serde_json::json!({"A?": "x"}))).unwrap();
        let ob = open_block(&conn, id).unwrap().unwrap();
        assert_eq!(ob.message_id, m);
        assert_eq!(ob.drafts["A?"], "x");
        mark_answered(&conn, m).unwrap();
        assert!(open_block(&conn, id).unwrap().is_none());
    }

    #[test]
    fn brief_title_drops_the_brief_prefix() {
        assert_eq!(brief_title("intro\n# Brief: Guest checkout\n## Goal"), "Guest checkout");
        assert_eq!(brief_title("# Shape Board grill"), "Shape Board grill");
        assert_eq!(brief_title("no heading"), "Untitled feature");
    }
}
