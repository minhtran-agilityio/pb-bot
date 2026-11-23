//! SQLite storage. The schema lives in `migrations/NNNN_*.sql`; each file runs once, in order,
//! and `PRAGMA user_version` records how many have run.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

/// Every migration, in order. Append new files here; never edit one that has shipped.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_projects.sql"),
    include_str!("../migrations/0002_artifacts.sql"),
];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub repo: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
}

pub fn open(file: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(file)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let done: usize = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(done) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn list_projects(conn: &Connection) -> rusqlite::Result<Vec<Project>> {
    let mut stmt =
        conn.prepare("SELECT id, repo, name, path, created_at FROM projects ORDER BY name COLLATE NOCASE")?;
    let rows = stmt.query_map([], row_to_project)?;
    rows.collect()
}

pub fn find_by_repo(conn: &Connection, repo: &str) -> rusqlite::Result<Option<Project>> {
    conn.query_row(
        "SELECT id, repo, name, path, created_at FROM projects WHERE repo = ?1 COLLATE NOCASE",
        params![repo],
        row_to_project,
    )
    .optional()
}

pub fn insert_project(conn: &Connection, repo: &str, name: &str, path: &str) -> rusqlite::Result<Project> {
    conn.execute(
        "INSERT INTO projects (repo, name, path) VALUES (?1, ?2, ?3)",
        params![repo, name, path],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, repo, name, path, created_at FROM projects WHERE id = ?1",
        params![id],
        row_to_project,
    )
}

fn row_to_project(r: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        repo: r.get(1)?,
        name: r.get(2)?,
        path: r.get(3)?,
        created_at: r.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_set_user_version_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("t.db");
        let conn = open(&file).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v as usize, MIGRATIONS.len());
        drop(conn);
        // Opening again must not re-run a migration (CREATE TABLE would fail).
        open(&file).unwrap();
    }

    #[test]
    fn a_project_survives_closing_and_reopening_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("t.db");
        {
            let conn = open(&file).unwrap();
            insert_project(&conn, "minhtran-agilityio/pb-bot", "pb-bot", "/tmp/pb-bot").unwrap();
        }
        let conn = open(&file).unwrap();
        let all = list_projects(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].repo, "minhtran-agilityio/pb-bot");
        assert_eq!(all[0].path, "/tmp/pb-bot");
    }

    #[test]
    fn a_step_1_database_migrates_to_step_2_and_keeps_its_projects() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("t.db");
        {
            // A database as build step 1 left it: only 0001 ran.
            let mut conn = Connection::open(&file).unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute_batch(MIGRATIONS[0]).unwrap();
            tx.pragma_update(None, "user_version", 1).unwrap();
            tx.commit().unwrap();
            insert_project(&conn, "a/b", "b", "/x").unwrap();
        }
        let conn = open(&file).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 2);
        assert_eq!(list_projects(&conn).unwrap().len(), 1);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn the_same_repo_cannot_be_added_twice() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("t.db")).unwrap();
        insert_project(&conn, "a/b", "b", "/x").unwrap();
        assert!(insert_project(&conn, "a/b", "b", "/y").is_err());
        assert!(find_by_repo(&conn, "A/B").unwrap().is_some());
    }
}
