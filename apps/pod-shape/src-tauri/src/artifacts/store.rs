//! SQLite side of artifacts: hook upserts, versions with the hash dedupe, the "New artifacts" inbox,
//! project auto-select and the manual (paste or drop) import.

use super::log::link_id;
use super::paths::MAX_HTML_BYTES;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

/// The text as stored: a leading UTF-8 BOM is dropped, so a dropped file and a hook file of the same
/// page hash the same.
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

pub fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Lower-case slug from a title: `Hook Test Page` -> `hook-test-page`. Empty when nothing is left.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 63 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// A manual artifact name: `[a-z0-9][a-z0-9-]{0,62}`.
pub fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

// ---------- hook side ----------

/// The name of a hook artifact: a slug of the title, else a slug of the id.
pub fn hook_name(title: &str, claude_id: &str) -> String {
    match slugify(title) {
        s if s.is_empty() => slugify(claude_id),
        s => s,
    }
}

/// Metadata of an artifact as the hook reports it.
pub struct HookMeta<'a> {
    pub claude_id: &'a str,
    pub link: Option<&'a str>,
    pub title: &'a str,
    pub cwd: Option<&'a str>,
    pub published_at: Option<&'a str>,
}

/// Finds or creates the artifact with this claude id. Hook metadata (title, cwd, link, publish time)
/// is refreshed; project and state are left as they are. The name is a slug of the title, else of
/// the id. A folder-first import (the hook moves the file before it appends the log line) leaves the
/// id as title and name; the log line then replaces both while the artifact is still in the inbox.
/// Returns the artifact row id.
pub fn upsert_hook_artifact(conn: &Connection, m: &HookMeta) -> rusqlite::Result<i64> {
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM artifacts WHERE claude_id = ?1", params![m.claude_id], |r| r.get(0))
        .optional()?;
    if let Some(id) = existing {
        conn.execute(
            "UPDATE artifacts SET
               name = CASE WHEN state = 'new' AND title = claude_id AND ?2 <> claude_id AND ?6 <> '' THEN ?6 ELSE name END,
               title = CASE WHEN ?2 <> claude_id THEN ?2 ELSE title END,
               cwd = COALESCE(?3, cwd),
               link = COALESCE(link, ?4),
               published_at = CASE WHEN ?5 IS NOT NULL AND (published_at IS NULL OR ?5 > published_at) THEN ?5 ELSE published_at END
             WHERE id = ?1",
            params![id, m.title, m.cwd, m.link, m.published_at, slugify(m.title)],
        )?;
        return Ok(id);
    }
    let name = hook_name(m.title, m.claude_id);
    let link = m.link.map(str::to_string).unwrap_or_else(|| format!("https://claude.ai/artifact/{}", m.claude_id));
    conn.execute(
        "INSERT INTO artifacts (project_id, claude_id, link, name, title, cwd, source, state, published_at)
         VALUES (NULL, ?1, ?2, ?3, ?4, ?5, 'hook', 'new', ?6)",
        params![m.claude_id, link, name, m.title, m.cwd, m.published_at],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Adds `html` as the next version of an artifact. Skipped (`None`) when the same hook file was
/// already imported, or when the content equals the latest version's (HANDOFF §11.4.4); a revert to an
/// older content (A, B, A) is a new version. Returns `Some((version_id, n))` for a new version. A new
/// version of an artifact that is in a project marks it "updated" (§11.4.5); nothing else happens.
pub fn add_version(
    conn: &Connection,
    artifact_id: i64,
    html: &str,
    source: &str,
    file: Option<&str>,
) -> rusqlite::Result<Option<(i64, i64)>> {
    let html = strip_bom(html);
    let hash = sha256_hex(html);
    if let Some(f) = file {
        let seen: i64 = conn.query_row("SELECT COUNT(*) FROM artifact_versions WHERE file = ?1", params![f], |r| r.get(0))?;
        if seen > 0 {
            return Ok(None);
        }
    }
    if latest_version(conn, artifact_id)?.is_some_and(|(_, _, h)| h == hash) {
        return Ok(None);
    }
    let n: i64 = conn.query_row(
        "SELECT COALESCE(MAX(n), 0) + 1 FROM artifact_versions WHERE artifact_id = ?1",
        params![artifact_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO artifact_versions (artifact_id, n, html, sha256, source, file) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![artifact_id, n, html, hash, source, file],
    )?;
    let vid = conn.last_insert_rowid();
    if n > 1 {
        conn.execute("UPDATE artifacts SET updated = 1 WHERE id = ?1 AND state = 'added'", params![artifact_id])?;
    }
    Ok(Some((vid, n)))
}

/// `(version_id, n, sha256)` of the newest version.
pub fn latest_version(conn: &Connection, artifact_id: i64) -> rusqlite::Result<Option<(i64, i64, String)>> {
    conn.query_row(
        "SELECT id, n, sha256 FROM artifact_versions WHERE artifact_id = ?1 ORDER BY n DESC LIMIT 1",
        params![artifact_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
}

pub fn get_state(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM hook_state WHERE key = ?1", params![key], |r| r.get(0)).optional()
}

pub fn set_state(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO hook_state (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// True when this file was handled before with the same size and mtime.
pub fn file_seen(conn: &Connection, path: &str, size: u64, mtime: i64) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM hook_files WHERE path = ?1 AND size = ?2 AND mtime = ?3",
        params![path, size as i64, mtime],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn mark_file(conn: &Connection, path: &str, size: u64, mtime: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO hook_files (path, size, mtime) VALUES (?1, ?2, ?3)
         ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime",
        params![path, size as i64, mtime],
    )?;
    Ok(())
}

// ---------- inbox and project auto-select ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: i64,
    pub title: String,
    pub link: Option<String>,
    pub published_at: Option<String>,
    pub cwd: Option<String>,
    pub versions: i64,
    pub suggested_project_id: Option<i64>,
}

/// The project whose folder holds `cwd`, by whole path components; the longest folder wins
/// (HANDOFF §11.4.3). `None` when no folder holds it.
pub fn suggest_project(projects: &[(i64, String)], cwd: Option<&str>) -> Option<i64> {
    let cwd = Path::new(cwd?);
    projects
        .iter()
        .filter(|(_, p)| cwd.starts_with(Path::new(p)))
        .max_by_key(|(_, p)| Path::new(p).components().count())
        .map(|(id, _)| *id)
}

fn project_paths(conn: &Connection) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut st = conn.prepare("SELECT id, path FROM projects")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Hook artifacts that are in no project and not ignored, newest first.
pub fn list_inbox(conn: &Connection) -> rusqlite::Result<Vec<InboxItem>> {
    let projects = project_paths(conn)?;
    let mut st = conn.prepare(
        "SELECT a.id, a.title, a.link, a.published_at, a.cwd,
                (SELECT COUNT(*) FROM artifact_versions v WHERE v.artifact_id = a.id)
         FROM artifacts a WHERE a.state = 'new' AND a.project_id IS NULL
           AND EXISTS (SELECT 1 FROM artifact_versions v WHERE v.artifact_id = a.id)
         ORDER BY COALESCE(a.published_at, a.created_at) DESC, a.id DESC",
    )?;
    let rows = st.query_map([], |r| {
        let cwd: Option<String> = r.get(4)?;
        Ok(InboxItem {
            id: r.get(0)?,
            title: r.get(1)?,
            link: r.get(2)?,
            published_at: r.get(3)?,
            suggested_project_id: suggest_project(&projects, cwd.as_deref()),
            cwd,
            versions: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// A name that no other artifact of the project uses: `name`, else `name-2`, `name-3` ...
fn free_name(conn: &Connection, project_id: i64, name: &str, except: i64) -> rusqlite::Result<String> {
    let mut candidate = name.to_string();
    let mut i = 2;
    loop {
        let taken: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artifacts WHERE project_id = ?1 AND name = ?2 AND id <> ?3",
            params![project_id, candidate, except],
            |r| r.get(0),
        )?;
        if taken == 0 {
            return Ok(candidate);
        }
        candidate = format!("{name}-{i}");
        i += 1;
    }
}

/// "Add to project" in the New artifacts box.
pub fn add_to_project(conn: &Connection, artifact_id: i64, project_id: i64) -> Result<(), String> {
    let row: Option<(String, Option<i64>)> = conn
        .query_row("SELECT name, project_id FROM artifacts WHERE id = ?1", params![artifact_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let (name, current) = row.ok_or("This artifact no longer exists.")?;
    if current.is_some() {
        return Err("This artifact is already in a project.".into());
    }
    let exists: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects WHERE id = ?1", params![project_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err("This project no longer exists.".into());
    }
    let name = free_name(conn, project_id, &name, artifact_id).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE artifacts SET project_id = ?2, name = ?3, state = 'added', updated = 0 WHERE id = ?1",
        params![artifact_id, project_id, name],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// "Ignore" in the New artifacts box. Versions are kept; later versions are stored but stay hidden.
pub fn ignore(conn: &Connection, artifact_id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE artifacts SET state = 'ignored' WHERE id = ?1 AND project_id IS NULL", params![artifact_id])?;
    Ok(())
}

// ---------- project page ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSummary {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub link: Option<String>,
    pub cwd: Option<String>,
    pub source: String,
    pub updated: bool,
    pub versions: i64,
    /// Publish time for a hook artifact, else the time of the newest version.
    pub latest_at: String,
}

pub fn list_artifacts(conn: &Connection, project_id: i64) -> rusqlite::Result<Vec<ArtifactSummary>> {
    let mut st = conn.prepare(
        "SELECT a.id, a.name, a.title, a.link, a.cwd, a.source, a.updated,
                COUNT(v.id),
                -- A hook artifact shows when it was published; a manual one when its newest version was saved.
                CASE WHEN a.source = 'hook' AND a.published_at IS NOT NULL THEN a.published_at
                     ELSE COALESCE(MAX(v.created_at), a.created_at) END
         FROM artifacts a LEFT JOIN artifact_versions v ON v.artifact_id = a.id
         WHERE a.project_id = ?1 GROUP BY a.id ORDER BY 9 DESC, a.id DESC",
    )?;
    let rows = st.query_map(params![project_id], |r| {
        Ok(ArtifactSummary {
            id: r.get(0)?,
            name: r.get(1)?,
            title: r.get(2)?,
            link: r.get(3)?,
            cwd: r.get(4)?,
            source: r.get(5)?,
            updated: r.get::<_, i64>(6)? != 0,
            versions: r.get(7)?,
            latest_at: r.get(8)?,
        })
    })?;
    rows.collect()
}

/// Artifact count per project, for the project cards.
pub fn artifact_counts(conn: &Connection) -> rusqlite::Result<Vec<(i64, i64)>> {
    let mut st = conn.prepare("SELECT project_id, COUNT(*) FROM artifacts WHERE project_id IS NOT NULL GROUP BY project_id")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub id: i64,
    pub n: i64,
    pub source: String,
    pub created_at: String,
    pub bytes: i64,
}

pub fn list_versions(conn: &Connection, artifact_id: i64) -> rusqlite::Result<Vec<VersionInfo>> {
    let mut st = conn.prepare(
        "SELECT id, n, source, created_at, LENGTH(CAST(html AS BLOB)) FROM artifact_versions
         WHERE artifact_id = ?1 ORDER BY n DESC",
    )?;
    let rows = st.query_map(params![artifact_id], |r| {
        Ok(VersionInfo { id: r.get(0)?, n: r.get(1)?, source: r.get(2)?, created_at: r.get(3)?, bytes: r.get(4)? })
    })?;
    rows.collect()
}

pub fn mark_seen(conn: &Connection, artifact_id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE artifacts SET updated = 0 WHERE id = ?1", params![artifact_id])?;
    Ok(())
}

/// The stored HTML of one version, for the preview scheme.
pub fn version_html(conn: &Connection, version_id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT html FROM artifact_versions WHERE id = ?1", params![version_id], |r| r.get(0))
        .optional()
}

// ---------- manual import (paste or drop) ----------

pub const LINK_ERROR: &str = "Paste a claude.ai artifact link, for example https://claude.ai/artifact/…";
pub const NAME_ERROR: &str = "Use lower-case letters, digits and dashes, for example settings-page.";
pub const HTML_ERROR: &str = "Paste the HTML of the artifact. In Claude, open the artifact and copy its code.";
pub const SIZE_ERROR: &str = "The HTML is larger than 5 MB. Shape Board stores files up to 5 MB.";

/// Where a manual import goes: an existing artifact (a new version) or a new one.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManualTarget {
    pub artifact_id: Option<i64>,
    pub next_version: i64,
}

/// Field errors of the manual import; empty when the input is fine.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

impl FieldErrors {
    pub fn is_empty(&self) -> bool {
        self.link.is_none() && self.name.is_none() && self.html.is_none()
    }
}

/// Finds the artifact a manual import adds to. The link wins over the name. An artifact of another
/// project is an error on the link field.
pub fn find_manual_target(
    conn: &Connection,
    project_id: i64,
    link: Option<&str>,
    name: &str,
) -> Result<ManualTarget, FieldErrors> {
    let db = |e: rusqlite::Error| FieldErrors { html: Some(e.to_string()), ..Default::default() };
    let next = |id: i64| -> rusqlite::Result<i64> {
        conn.query_row("SELECT COALESCE(MAX(n), 0) + 1 FROM artifact_versions WHERE artifact_id = ?1", params![id], |r| r.get(0))
    };
    if let Some(link) = link {
        let cid = link_id(link).ok_or(FieldErrors { link: Some(LINK_ERROR.into()), ..Default::default() })?;
        let hit: Option<(i64, Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT a.id, a.project_id, p.name FROM artifacts a LEFT JOIN projects p ON p.id = a.project_id
                 WHERE a.claude_id = ?1",
                params![cid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db)?;
        if let Some((id, pid, pname)) = hit {
            if pid.is_some_and(|p| p != project_id) {
                return Err(FieldErrors {
                    link: Some(format!("This artifact is in project {}.", pname.unwrap_or_default())),
                    ..Default::default()
                });
            }
            return Ok(ManualTarget { artifact_id: Some(id), next_version: next(id).map_err(db)? });
        }
    }
    let by_name: Option<i64> = conn
        .query_row("SELECT id FROM artifacts WHERE project_id = ?1 AND name = ?2", params![project_id, name], |r| r.get(0))
        .optional()
        .map_err(db)?;
    match by_name {
        Some(id) => Ok(ManualTarget { artifact_id: Some(id), next_version: next(id).map_err(db)? }),
        None => Ok(ManualTarget { artifact_id: None, next_version: 1 }),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManualResult {
    pub artifact_id: i64,
    pub version_id: i64,
    pub n: i64,
    /// False when the HTML equals the latest version (nothing new was saved).
    pub created: bool,
}

/// Checks the fields of a manual import. A paste needs the link; a drop may leave it empty.
pub fn check_manual(link: Option<&str>, name: &str, html: &str, source: &str) -> FieldErrors {
    let mut e = FieldErrors::default();
    match link {
        Some(l) if link_id(l).is_none() => e.link = Some(LINK_ERROR.into()),
        None if source == "paste" => e.link = Some(LINK_ERROR.into()),
        _ => {}
    }
    if !valid_name(name) {
        e.name = Some(NAME_ERROR.into());
    }
    let html = strip_bom(html);
    if html.len() as u64 > MAX_HTML_BYTES {
        e.html = Some(SIZE_ERROR.into());
    } else if html.trim().is_empty() || !html.contains('<') {
        e.html = Some(HTML_ERROR.into());
    }
    e
}

/// Saves a pasted or dropped HTML into a project (HANDOFF §11.4.6). One transaction.
pub fn import_manual(
    conn: &mut Connection,
    project_id: i64,
    link: Option<&str>,
    name: &str,
    html: &str,
    source: &str,
) -> Result<ManualResult, FieldErrors> {
    if source != "paste" && source != "drop" {
        return Err(FieldErrors { html: Some("Unknown source.".into()), ..Default::default() });
    }
    let errors = check_manual(link, name, html, source);
    if !errors.is_empty() {
        return Err(errors);
    }
    let db = |e: rusqlite::Error| FieldErrors { html: Some(e.to_string()), ..Default::default() };
    let tx = conn.transaction().map_err(db)?;
    let pexists: i64 = tx
        .query_row("SELECT COUNT(*) FROM projects WHERE id = ?1", params![project_id], |r| r.get(0))
        .map_err(db)?;
    if pexists == 0 {
        return Err(FieldErrors { html: Some("This project no longer exists.".into()), ..Default::default() });
    }
    let target = find_manual_target(&tx, project_id, link, name)?;
    let artifact_id = match target.artifact_id {
        Some(id) => {
            // A hook artifact from the inbox that the owner adds by hand joins this project.
            let pid: Option<i64> =
                tx.query_row("SELECT project_id FROM artifacts WHERE id = ?1", params![id], |r| r.get(0)).map_err(db)?;
            if pid.is_none() {
                add_to_project(&tx, id, project_id).map_err(|m| FieldErrors { html: Some(m), ..Default::default() })?;
            }
            id
        }
        None => {
            let cid = link.and_then(link_id);
            tx.execute(
                "INSERT INTO artifacts (project_id, claude_id, link, name, title, source, state)
                 VALUES (?1, ?2, ?3, ?4, ?4, 'manual', 'added')",
                params![project_id, cid, link, name],
            )
            .map_err(db)?;
            tx.last_insert_rowid()
        }
    };
    let result = match add_version(&tx, artifact_id, html, source, None).map_err(db)? {
        Some((vid, n)) => ManualResult { artifact_id, version_id: vid, n, created: true },
        None => {
            let (vid, n, _) = latest_version(&tx, artifact_id).map_err(db)?.ok_or_else(|| db(rusqlite::Error::QueryReturnedNoRows))?;
            ManualResult { artifact_id, version_id: vid, n, created: false }
        }
    };
    // The owner just saved it by hand, so there is nothing "updated" to point out.
    tx.execute("UPDATE artifacts SET updated = 0 WHERE id = ?1", params![artifact_id]).map_err(db)?;
    tx.commit().map_err(db)?;
    Ok(result)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db;

    pub fn conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = db::open(&dir.path().join("t.db")).unwrap();
        (dir, c)
    }

    fn meta<'a>(id: &'a str, title: &'a str, cwd: Option<&'a str>) -> HookMeta<'a> {
        HookMeta { claude_id: id, link: None, title, cwd, published_at: Some("2026-11-22T11:13:40Z") }
    }

    #[test]
    fn slugs_and_names() {
        assert_eq!(slugify("Hook Test Page"), "hook-test-page");
        assert_eq!(slugify("  --Ünïcode!! v2 "), "n-code-v2");
        assert_eq!(slugify("!!!"), "");
        assert!(valid_name("settings-page"));
        assert!(valid_name("2fa"));
        for bad in ["", "-a", "A", "a b", "a/b", "..", &"a".repeat(64)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn versions_dedupe_by_hash_and_count_up() {
        let (_d, c) = conn();
        let a = upsert_hook_artifact(&c, &meta("A1", "Page", None)).unwrap();
        assert_eq!(add_version(&c, a, "<p>1</p>", "hook", None).unwrap().unwrap().1, 1);
        assert_eq!(add_version(&c, a, "<p>1</p>", "hook", None).unwrap(), None, "same content is skipped");
        assert_eq!(add_version(&c, a, "<p>2</p>", "hook", None).unwrap().unwrap().1, 2);
        assert_eq!(list_versions(&c, a).unwrap().iter().map(|v| v.n).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn a_revert_a_b_a_is_three_versions_and_a_file_is_imported_once() {
        let (_d, c) = conn();
        let a = upsert_hook_artifact(&c, &meta("A1", "Page", None)).unwrap();
        assert!(add_version(&c, a, "A", "hook", Some("/r/A1/1.html")).unwrap().is_some());
        assert!(add_version(&c, a, "B", "hook", Some("/r/A1/2.html")).unwrap().is_some());
        assert_eq!(add_version(&c, a, "A", "hook", Some("/r/A1/3.html")).unwrap().unwrap().1, 3);
        assert_eq!(add_version(&c, a, "C", "hook", Some("/r/A1/3.html")).unwrap(), None, "same file again");
        assert_eq!(add_version(&c, a, "A", "hook", Some("/r/A1/4.html")).unwrap(), None, "same as the latest");
    }

    #[test]
    fn a_bom_is_stripped_so_drop_and_hook_hash_the_same() {
        let (_d, mut c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        let a = upsert_hook_artifact(&c, &meta("Bom1", "Page", None)).unwrap();
        add_version(&c, a, "\u{feff}<p>x</p>", "hook", Some("/r/Bom1/1.html")).unwrap();
        assert_eq!(version_html(&c, 1).unwrap().unwrap(), "<p>x</p>");
        add_to_project(&c, a, p.id).unwrap();
        let r = import_manual(&mut c, p.id, Some("https://claude.ai/artifact/Bom1"), "page", "<p>x</p>", "drop").unwrap();
        assert_eq!((r.n, r.created), (1, false));
    }

    #[test]
    fn the_state_check_ties_added_to_a_project() {
        let (_d, c) = conn();
        assert!(c
            .execute("INSERT INTO artifacts (name, source, state) VALUES ('x', 'hook', 'added')", [])
            .is_err());
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        assert!(c
            .execute("INSERT INTO artifacts (project_id, name, source, state) VALUES (?1, 'x', 'hook', 'new')", [p.id])
            .is_err());
    }

    #[test]
    fn a_log_title_replaces_the_placeholder_of_a_folder_first_import() {
        let (_d, c) = conn();
        let a = upsert_hook_artifact(&c, &meta("Fold1", "Fold1", None)).unwrap();
        upsert_hook_artifact(&c, &meta("Fold1", "Real Title", Some("/u/x"))).unwrap();
        let (name, title, cwd): (String, String, Option<String>) = c
            .query_row("SELECT name, title, cwd FROM artifacts WHERE id = ?1", [a], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!((name.as_str(), title.as_str(), cwd.as_deref()), ("real-title", "Real Title", Some("/u/x")));
    }

    #[test]
    fn a_new_version_marks_an_added_artifact_updated_only() {
        let (_d, c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/code/r").unwrap();
        let a = upsert_hook_artifact(&c, &meta("A1", "Page", None)).unwrap();
        add_version(&c, a, "v1", "hook", None).unwrap();
        add_version(&c, a, "v2", "hook", None).unwrap();
        let updated = |c: &Connection| -> i64 { c.query_row("SELECT updated FROM artifacts WHERE id = ?1", [a], |r| r.get(0)).unwrap() };
        assert_eq!(updated(&c), 0, "inbox artifacts are not marked");
        add_to_project(&c, a, p.id).unwrap();
        assert_eq!(updated(&c), 0);
        add_version(&c, a, "v3", "hook", None).unwrap();
        assert_eq!(updated(&c), 1);
        assert!(list_artifacts(&c, p.id).unwrap()[0].updated);
        mark_seen(&c, a).unwrap();
        assert_eq!(updated(&c), 0);
    }

    #[test]
    fn auto_select_picks_the_longest_folder_by_whole_components() {
        let ps = vec![(1, "/u/code".to_string()), (2, "/u/code/pb-bot".to_string()), (3, "/u/code/pb-bot2".to_string())];
        assert_eq!(suggest_project(&ps, Some("/u/code/pb-bot/apps/x")), Some(2));
        assert_eq!(suggest_project(&ps, Some("/u/code/pb-bot")), Some(2));
        assert_eq!(suggest_project(&ps, Some("/u/code/pb-bot2")), Some(3));
        assert_eq!(suggest_project(&ps, Some("/u/code/other")), Some(1));
        assert_eq!(suggest_project(&ps, Some("/u/codex")), None);
        assert_eq!(suggest_project(&ps, Some("/elsewhere")), None);
        assert_eq!(suggest_project(&ps, None), None);
        assert_eq!(suggest_project(&[], Some("/u/code")), None);
    }

    #[test]
    fn inbox_add_and_ignore() {
        let (_d, c) = conn();
        let p = db::insert_project(&c, "o/pb-bot", "pb-bot", "/u/code/pb-bot").unwrap();
        let a = upsert_hook_artifact(&c, &meta("A1", "Hook Test Page", Some("/u/code/pb-bot/apps"))).unwrap();
        let b = upsert_hook_artifact(&c, &meta("B2", "Other", Some("/tmp"))).unwrap();
        let empty = upsert_hook_artifact(&c, &meta("C3", "No file yet", None)).unwrap();
        add_version(&c, a, "a", "hook", None).unwrap();
        add_version(&c, b, "b", "hook", None).unwrap();
        let inbox = list_inbox(&c).unwrap();
        assert_eq!(inbox.len(), 2, "an artifact with no stored version is not offered");
        assert!(inbox.iter().all(|i| i.id != empty));
        assert_eq!(inbox.iter().find(|i| i.id == a).unwrap().suggested_project_id, Some(p.id));
        assert_eq!(inbox.iter().find(|i| i.id == b).unwrap().suggested_project_id, None);

        add_to_project(&c, a, p.id).unwrap();
        ignore(&c, b).unwrap();
        assert!(list_inbox(&c).unwrap().is_empty());
        assert_eq!(list_artifacts(&c, p.id).unwrap()[0].name, "hook-test-page");
        assert_eq!(list_artifacts(&c, p.id).unwrap()[0].latest_at, "2026-11-22T11:13:40Z", "publish time, not import time");
        assert!(add_to_project(&c, a, p.id).is_err(), "already in a project");
        assert!(add_to_project(&c, b, 999).is_err(), "unknown project");
        // An ignored artifact stays hidden when a new version arrives.
        add_version(&c, b, "b2", "hook", None).unwrap();
        assert!(list_inbox(&c).unwrap().is_empty());
        assert_eq!(artifact_counts(&c).unwrap(), vec![(p.id, 1)]);
    }

    #[test]
    fn hook_metadata_refresh_keeps_project_and_name() {
        let (_d, c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        let a = upsert_hook_artifact(&c, &meta("A1", "First", None)).unwrap();
        add_version(&c, a, "x", "hook", None).unwrap();
        add_to_project(&c, a, p.id).unwrap();
        let again = upsert_hook_artifact(
            &c,
            &HookMeta { claude_id: "A1", link: None, title: "Renamed", cwd: Some("/r"), published_at: Some("2026-11-23T00:00:00Z") },
        )
        .unwrap();
        assert_eq!(again, a);
        let s = &list_artifacts(&c, p.id).unwrap()[0];
        assert_eq!((s.name.as_str(), s.title.as_str()), ("first", "Renamed"));
    }

    #[test]
    fn names_get_a_suffix_when_taken_in_the_project() {
        let (_d, c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        for id in ["A1", "A2"] {
            let a = upsert_hook_artifact(&c, &meta(id, "Same title", None)).unwrap();
            add_version(&c, a, id, "hook", None).unwrap();
            add_to_project(&c, a, p.id).unwrap();
        }
        let mut names: Vec<_> = list_artifacts(&c, p.id).unwrap().into_iter().map(|a| a.name).collect();
        names.sort();
        assert_eq!(names, vec!["same-title", "same-title-2"]);
    }

    #[test]
    fn manual_import_paste_versions_and_errors() {
        let (_d, mut c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        let q = db::insert_project(&c, "o/q", "q", "/q").unwrap();
        let link = "https://claude.ai/artifact/Manual1";
        let r = import_manual(&mut c, p.id, Some(link), "settings-page", "<p>1</p>", "paste").unwrap();
        assert_eq!((r.n, r.created), (1, true));
        // Same name, new HTML, no link: a new version of the same artifact.
        let r2 = import_manual(&mut c, p.id, None, "settings-page", "<p>2</p>", "drop").unwrap();
        assert_eq!((r2.artifact_id, r2.n), (r.artifact_id, 2));
        // Same HTML as the latest again: nothing new.
        let r3 = import_manual(&mut c, p.id, Some(link), "other-name", "<p>2</p>", "paste").unwrap();
        assert_eq!((r3.artifact_id, r3.n, r3.created), (r.artifact_id, 2, false));
        assert_eq!(find_manual_target(&c, p.id, Some(link), "x").unwrap().next_version, 3);
        // The same link from another project is refused on the link field.
        let e = import_manual(&mut c, q.id, Some(link), "x", "<p>", "paste").unwrap_err();
        assert_eq!(e.link.unwrap(), "This artifact is in project r.");

        let e = import_manual(&mut c, p.id, None, "x", "<p>", "paste").unwrap_err();
        assert_eq!(e.link.as_deref(), Some(LINK_ERROR), "a paste needs the link");
        let e = import_manual(&mut c, p.id, Some("https://evil.example/artifact/x"), "Bad Name", "no tags", "paste").unwrap_err();
        assert_eq!((e.link.is_some(), e.name.is_some(), e.html.as_deref()), (true, true, Some(HTML_ERROR)));
        let big = format!("<p>{}", "a".repeat(MAX_HTML_BYTES as usize));
        assert_eq!(import_manual(&mut c, p.id, None, "big", &big, "drop").unwrap_err().html.as_deref(), Some(SIZE_ERROR));
        assert!(import_manual(&mut c, p.id, None, "x", "<p>", "evil").is_err());
        assert!(import_manual(&mut c, 999, None, "x", "<p>", "drop").is_err());
    }

    #[test]
    fn pasting_the_link_of_an_ignored_artifact_moves_it_into_the_project() {
        let (_d, mut c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        let a = upsert_hook_artifact(&c, &meta("Ign1", "Ignored", None)).unwrap();
        add_version(&c, a, "<p>1</p>", "hook", None).unwrap();
        ignore(&c, a).unwrap();
        let r = import_manual(&mut c, p.id, Some("https://claude.ai/artifact/Ign1"), "x", "<p>2</p>", "paste").unwrap();
        assert_eq!((r.artifact_id, r.n), (a, 2));
        assert_eq!(list_artifacts(&c, p.id).unwrap()[0].name, "ignored");
    }

    #[test]
    fn a_name_collision_on_move_gets_a_suffix() {
        let (_d, mut c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        import_manual(&mut c, p.id, None, "proto", "<p>m</p>", "drop").unwrap();
        let a = upsert_hook_artifact(&c, &meta("Col1", "Proto", None)).unwrap();
        add_version(&c, a, "<p>h</p>", "hook", None).unwrap();
        add_to_project(&c, a, p.id).unwrap();
        let mut names: Vec<_> = list_artifacts(&c, p.id).unwrap().into_iter().map(|x| x.name).collect();
        names.sort();
        assert_eq!(names, vec!["proto", "proto-2"]);
    }

    #[test]
    fn a_manual_paste_of_an_inbox_link_moves_it_into_the_project() {
        let (_d, mut c) = conn();
        let p = db::insert_project(&c, "o/r", "r", "/r").unwrap();
        let a = upsert_hook_artifact(&c, &meta("Hook1", "Hooked", None)).unwrap();
        add_version(&c, a, "<p>hook</p>", "hook", None).unwrap();
        let r = import_manual(&mut c, p.id, Some("https://claude.ai/artifact/Hook1"), "whatever", "<p>pasted</p>", "paste").unwrap();
        assert_eq!((r.artifact_id, r.n), (a, 2));
        assert!(list_inbox(&c).unwrap().is_empty());
        assert_eq!(list_artifacts(&c, p.id).unwrap().len(), 1);
    }
}
