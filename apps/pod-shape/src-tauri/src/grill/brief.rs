//! Brief detection by the file, not by the tool call (a Write can be denied or fail; advisor should-fix):
//! the app hashes every `specs/*/brief.md` of the project before a turn, and after it takes the one that
//! is new or changed. Only a regular file that really lies under `<project>/specs/` counts.

use crate::artifacts::store::sha256_hex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The largest brief the app reads.
pub const MAX_BRIEF_BYTES: u64 = 1024 * 1024;

/// `specs/<slug>/brief.md` (relative) -> sha256 of its text.
pub type Snapshot = HashMap<String, String>;

#[derive(Debug, Clone, PartialEq)]
pub struct Brief {
    /// Relative to the project: `specs/<slug>/brief.md`.
    pub path: String,
    pub content: String,
    pub sha: String,
}

/// Reads one candidate. `None` when it is not a regular file under `<project>/specs/` after
/// resolving links, is too large, or is not UTF-8.
fn read_brief(project: &Path, specs: &Path, slug_dir: &Path) -> Option<Brief> {
    let file = slug_dir.join("brief.md");
    let meta = std::fs::symlink_metadata(&file).ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_BRIEF_BYTES {
        return None;
    }
    let real = file.canonicalize().ok()?;
    if !real.starts_with(specs) {
        return None;
    }
    let content = String::from_utf8(std::fs::read(&real).ok()?).ok()?;
    let rel = real.strip_prefix(project).ok()?.to_string_lossy().into_owned();
    let sha = sha256_hex(&content);
    Some(Brief { path: rel, content, sha })
}

fn candidates(project: &Path) -> Vec<(PathBuf, Brief)> {
    let Ok(project) = project.canonicalize() else { return vec![] };
    let specs = project.join("specs");
    // `specs` itself must be a real folder in the project, not a link to somewhere else.
    match std::fs::symlink_metadata(&specs) {
        Ok(m) if m.file_type().is_dir() => {}
        _ => return vec![],
    }
    let Ok(entries) = std::fs::read_dir(&specs) else { return vec![] };
    let mut out = vec![];
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() {
            continue; // a symlinked slug folder is skipped too
        }
        if let Some(b) = read_brief(&project, &specs, &e.path()) {
            out.push((e.path().join("brief.md"), b));
        }
    }
    out
}

pub fn snapshot(project: &Path) -> Snapshot {
    candidates(project).into_iter().map(|(_, b)| (b.path, b.sha)).collect()
}

/// The brief this turn wrote: new or changed since `before`. With several, the newest file wins.
pub fn changed_since(project: &Path, before: &Snapshot) -> Option<Brief> {
    candidates(project)
        .into_iter()
        .filter(|(_, b)| before.get(&b.path) != Some(&b.sha))
        .max_by_key(|(p, _)| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .map(|(_, b)| b)
}

/// The current text of a brief the session already has (it may change without a new turn writing it).
pub fn read_current(project: &Path, rel: &str) -> Option<Brief> {
    let project = project.canonicalize().ok()?;
    let specs = project.join("specs");
    let dir = project.join(rel).parent()?.to_path_buf();
    read_brief(&project, &specs, &dir)
}

/// Every file under `<project>/specs/` (relative path -> (size, mtime ns)), at most 5 levels and 5 000
/// files, links not followed. Used to flag a turn that changed anything other than a brief or a design
/// (review F1: the permission rules deny it; this is the second line).
pub type SpecsFiles = HashMap<String, (u64, u128)>;

pub fn specs_files(project: &Path) -> SpecsFiles {
    let mut out = SpecsFiles::new();
    let Ok(project) = project.canonicalize() else { return out };
    let mut stack = vec![(project.join("specs"), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            if out.len() >= 5000 {
                return out;
            }
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() && depth < 5 {
                stack.push((e.path(), depth + 1));
            } else if m.is_file() || m.file_type().is_symlink() {
                let rel = e.path().strip_prefix(&project).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                let mtime = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
                out.insert(rel, (m.len(), mtime));
            }
        }
    }
    out
}

/// Files under specs/ that are new, changed or gone since `before`, other than this feature's
/// `specs/<slug>/brief.md` and `design.md`. With no slug yet, a NEW brief or design in any slug folder is
/// expected (pd-shape picks the slug); changing another feature's existing one is not. Sorted.
pub fn unexpected_changes(before: &SpecsFiles, after: &SpecsFiles, slug: Option<&str>) -> Vec<String> {
    let allowed = |p: &str| {
        let parts: Vec<&str> = p.split('/').collect();
        let shape = parts.len() == 3 && parts[0] == "specs" && (parts[2] == "brief.md" || parts[2] == "design.md");
        shape && match slug {
            Some(s) => parts[1] == s,
            None => !before.contains_key(p),
        }
    };
    let mut v: Vec<String> = after
        .iter()
        .filter(|(p, m)| before.get(*p) != Some(m))
        .map(|(p, _)| p.clone())
        .chain(before.keys().filter(|p| !after.contains_key(*p)).cloned())
        .filter(|p| !allowed(p))
        .collect();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_new_or_changed_brief_is_found_and_an_unchanged_one_is_not() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("specs/old")).unwrap();
        fs::write(p.join("specs/old/brief.md"), "# Brief: Old").unwrap();
        let before = snapshot(p);
        assert_eq!(before.len(), 1);
        assert!(changed_since(p, &before).is_none());
        fs::create_dir_all(p.join("specs/phone-sign-up")).unwrap();
        fs::write(p.join("specs/phone-sign-up/brief.md"), "# Brief: Phone sign-up").unwrap();
        let b = changed_since(p, &before).unwrap();
        assert_eq!(b.path, "specs/phone-sign-up/brief.md");
        assert_eq!(b.content, "# Brief: Phone sign-up");
        let before = snapshot(p);
        fs::write(p.join("specs/phone-sign-up/brief.md"), "# Brief: Phone sign-up\nv2").unwrap();
        assert_eq!(changed_since(p, &before).unwrap().content, "# Brief: Phone sign-up\nv2");
        assert_eq!(read_current(p, "specs/phone-sign-up/brief.md").unwrap().content, "# Brief: Phone sign-up\nv2");
    }

    #[test]
    fn only_briefs_and_designs_may_change_under_specs() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("specs/a")).unwrap();
        fs::write(p.join("specs/a/plan.md"), "old plan").unwrap();
        fs::write(p.join("specs/a/gone.md"), "x").unwrap();
        let before = specs_files(p);
        fs::write(p.join("specs/a/brief.md"), "# Brief").unwrap();
        fs::write(p.join("specs/a/design.md"), "# Design").unwrap();
        fs::write(p.join("specs/a/plan.md"), "planted plan, longer").unwrap();
        fs::create_dir_all(p.join("specs/b/deep")).unwrap();
        fs::write(p.join("specs/b/deep/brief.md"), "nested").unwrap();
        fs::remove_file(p.join("specs/a/gone.md")).unwrap();
        let after = specs_files(p);
        assert_eq!(unexpected_changes(&before, &after, None), vec!["specs/a/gone.md", "specs/a/plan.md", "specs/b/deep/brief.md"]);
        assert!(unexpected_changes(&after, &specs_files(p), None).is_empty());
        // Once the feature is "a", another feature's brief is flagged, even a new one; a's own is not.
        let before = specs_files(p);
        fs::create_dir_all(p.join("specs/other")).unwrap();
        fs::write(p.join("specs/other/brief.md"), "planted").unwrap();
        fs::write(p.join("specs/a/brief.md"), "# Brief v2, longer").unwrap();
        assert_eq!(unexpected_changes(&before, &specs_files(p), Some("a")), vec!["specs/other/brief.md"]);
        // With no slug yet, overwriting an EXISTING brief of another feature is flagged.
        let before = specs_files(p);
        fs::write(p.join("specs/other/brief.md"), "planted again, longer").unwrap();
        assert_eq!(unexpected_changes(&before, &specs_files(p), None), vec!["specs/other/brief.md"]);
    }

    #[test]
    fn links_out_of_specs_and_large_files_are_ignored() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("repo");
        let outside = d.path().join("outside");
        fs::create_dir_all(p.join("specs/a")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("brief.md"), "secret").unwrap();
        // A brief.md that links out of specs/.
        std::os::unix::fs::symlink(outside.join("brief.md"), p.join("specs/a/brief.md")).unwrap();
        // A slug folder that is a link.
        std::os::unix::fs::symlink(&outside, p.join("specs/b")).unwrap();
        // Too large.
        fs::create_dir_all(p.join("specs/c")).unwrap();
        fs::write(p.join("specs/c/brief.md"), vec![b'x'; (MAX_BRIEF_BYTES + 1) as usize]).unwrap();
        assert!(changed_since(&p, &Snapshot::new()).is_none());
        // specs itself as a link.
        let p2 = d.path().join("repo2");
        fs::create_dir_all(&p2).unwrap();
        fs::create_dir_all(outside.join("x")).unwrap();
        fs::write(outside.join("x/brief.md"), "secret").unwrap();
        std::os::unix::fs::symlink(&outside, p2.join("specs")).unwrap();
        assert!(changed_since(&p2, &Snapshot::new()).is_none());
        assert!(read_current(&p, "specs/a/brief.md").is_none());
    }
}
