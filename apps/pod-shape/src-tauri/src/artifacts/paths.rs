//! Where the artifact hook writes (HANDOFF §11.3) and the guard every hook file passes before it is read.
//!
//! Rule: Rust reads only regular `.html` files that sit directly in `<root>/<artifactId>/`, at most
//! 5 MiB. Neither the `<artifactId>` folder nor the file may be a symlink (a symlinked root is fine).
//! The path is built from the root, a checked id and one file name, never from a log column.
//! Files are opened with `O_NOFOLLOW | O_NONBLOCK` and checked again on the open handle, so a swap to
//! a symlink or a FIFO between the check and the read cannot redirect or hang the reader. The handle
//! must also be the same file (device and inode) the guard saw, so swapping the `<id>` folder for a
//! symlink after the check cannot redirect the read either (security review L1).

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The largest HTML file or paste the app accepts (HANDOFF §11.4.7).
pub const MAX_HTML_BYTES: u64 = 5 * 1024 * 1024;

/// The two hook paths: the artifact folder and the log.
#[derive(Debug, Clone)]
pub struct HookPaths {
    pub root: PathBuf,
    pub log: PathBuf,
}

impl HookPaths {
    /// `~/claude-artifacts` and `~/.claude/artifact-log.tsv`. Debug builds accept
    /// `SHAPE_BOARD_ARTIFACTS_DIR` and `SHAPE_BOARD_ARTIFACT_LOG` (fixture demo); release builds ignore them.
    pub fn for_home(home: &Path) -> HookPaths {
        let mut p = HookPaths {
            root: home.join("claude-artifacts"),
            log: home.join(".claude").join("artifact-log.tsv"),
        };
        if cfg!(debug_assertions) {
            if let Some(v) = std::env::var_os("SHAPE_BOARD_ARTIFACTS_DIR") {
                p.root = PathBuf::from(v);
            }
            if let Some(v) = std::env::var_os("SHAPE_BOARD_ARTIFACT_LOG") {
                p.log = PathBuf::from(v);
            }
        }
        p
    }
}

/// An artifact id: the last part of the claude.ai link and the folder name. `[A-Za-z0-9_-]{1,64}`,
/// so it can never be `..`, hold a `/`, or be empty.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Why a hook file was not read.
#[derive(Debug, Clone, PartialEq)]
pub enum Rejected {
    BadId,
    BadName,
    Missing,
    Symlink,
    NotHtml,
    NotAFile,
    TooLarge,
    /// Size or mtime changed while it was read: try again on the next sync.
    Changed,
}

/// A hook file that passed the guard: its path, size and mtime (seconds).
#[derive(Debug, Clone, PartialEq)]
pub struct GuardedFile {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
    /// Device and inode the guard saw; the open handle must match them.
    pub dev: u64,
    pub ino: u64,
}

/// True when the file name ends in `.html` (any case) and is not a `.tmp` file.
pub fn is_html_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".html") && lower.len() > ".html".len()
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `<root>/<id>` when it is a real folder (not a symlink). The root itself may be a symlink.
pub fn artifact_dir(root: &Path, id: &str) -> Result<PathBuf, Rejected> {
    if !valid_id(id) {
        return Err(Rejected::BadId);
    }
    let dir = root.join(id);
    let meta = std::fs::symlink_metadata(&dir).map_err(|_| Rejected::Missing)?;
    if meta.file_type().is_symlink() {
        return Err(Rejected::Symlink);
    }
    if !meta.is_dir() {
        return Err(Rejected::Missing);
    }
    Ok(dir)
}

/// Accepts `<root>/<id>/<name>` only when the id is valid, `name` is one plain file name ending in
/// `.html`, neither the folder nor the file is a symlink, it is a regular file and at most 5 MiB.
pub fn guard(root: &Path, id: &str, name: &str) -> Result<GuardedFile, Rejected> {
    let dir = artifact_dir(root, id)?;
    if name.is_empty() || name.contains('/') || name.contains('\0') || name == "." || name == ".." {
        return Err(Rejected::BadName);
    }
    if !is_html_name(name) {
        return Err(Rejected::NotHtml);
    }
    let path = dir.join(name);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| Rejected::Missing)?;
    if meta.file_type().is_symlink() {
        return Err(Rejected::Symlink);
    }
    if !meta.is_file() {
        return Err(Rejected::NotAFile);
    }
    if meta.len() > MAX_HTML_BYTES {
        return Err(Rejected::TooLarge);
    }
    Ok(GuardedFile { path, size: meta.len(), mtime: mtime_of(&meta), dev: meta.dev(), ino: meta.ino() })
}

/// Reads a guarded file as text. The handle is opened without following a symlink and without
/// blocking on a FIFO; `fstat` on the handle must still show the same regular file, size and mtime,
/// before and after the read. Bytes that are not UTF-8 become U+FFFD.
pub fn read_guarded(f: &GuardedFile) -> Result<String, Rejected> {
    let mut file: File = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&f.path)
        .map_err(|e| if e.raw_os_error() == Some(libc::ELOOP) { Rejected::Symlink } else { Rejected::Missing })?;
    let before = file.metadata().map_err(|_| Rejected::Missing)?;
    if !before.is_file() {
        return Err(Rejected::NotAFile);
    }
    if before.len() > MAX_HTML_BYTES {
        return Err(Rejected::TooLarge);
    }
    if before.dev() != f.dev || before.ino() != f.ino {
        // Not the file the guard checked: a folder or file was swapped in between.
        return Err(Rejected::Changed);
    }
    if before.len() != f.size || mtime_of(&before) != f.mtime {
        return Err(Rejected::Changed);
    }
    let mut buf = Vec::new();
    (&mut file).take(MAX_HTML_BYTES + 1).read_to_end(&mut buf).map_err(|_| Rejected::Missing)?;
    let after = file.metadata().map_err(|_| Rejected::Missing)?;
    if buf.len() as u64 != f.size || after.len() != f.size || mtime_of(&after) != f.mtime {
        return Err(if buf.len() as u64 > MAX_HTML_BYTES { Rejected::TooLarge } else { Rejected::Changed });
    }
    Ok(match String::from_utf8(buf) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("claude-artifacts");
        fs::create_dir_all(root.join("AbC123")).unwrap();
        fs::create_dir_all(root.join("Other")).unwrap();
        (dir, root)
    }

    #[test]
    fn ids_cannot_escape_the_folder() {
        assert!(valid_id("XD26T57sEMLh28o2cF6j1J"));
        assert!(valid_id("a-b_c"));
        for bad in ["", "..", ".", "a/b", "a\\b", "a%2Fb", "a b", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn a_good_file_passes() {
        let (_d, root) = setup();
        fs::write(root.join("AbC123/20261122T111340Z_hook-test.html"), "<html></html>").unwrap();
        let g = guard(&root, "AbC123", "20261122T111340Z_hook-test.html").unwrap();
        assert_eq!(g.size, 13);
        assert_eq!(read_guarded(&g).unwrap(), "<html></html>");
        fs::write(root.join("AbC123/v2_X.HTML"), "<p>").unwrap();
        assert!(guard(&root, "AbC123", "v2_X.HTML").is_ok(), "upper-case extension");
    }

    #[test]
    fn names_and_ids_cannot_traverse() {
        let (d, root) = setup();
        fs::write(d.path().join("secret.html"), "secret").unwrap();
        assert_eq!(guard(&root, "AbC123", "../../secret.html").unwrap_err(), Rejected::BadName);
        assert_eq!(guard(&root, "AbC123", "..").unwrap_err(), Rejected::BadName);
        assert_eq!(guard(&root, "..", "secret.html").unwrap_err(), Rejected::BadId);
        assert_eq!(guard(&root, "AbC123", "none.html").unwrap_err(), Rejected::Missing);
        assert_eq!(guard(&root, "Nope", "x.html").unwrap_err(), Rejected::Missing);
    }

    #[test]
    fn symlinked_files_and_id_folders_are_rejected_but_a_symlinked_root_is_fine() {
        let (d, root) = setup();
        let outside = d.path().join("passwd.html");
        fs::write(&outside, "x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("AbC123/link.html")).unwrap();
        assert_eq!(guard(&root, "AbC123", "link.html").unwrap_err(), Rejected::Symlink);
        // A symlinked <id> folder.
        std::os::unix::fs::symlink(d.path(), root.join("Linked")).unwrap();
        assert_eq!(guard(&root, "Linked", "passwd.html").unwrap_err(), Rejected::Symlink);
        // A symlinked root is allowed.
        let alias = d.path().join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        fs::write(root.join("AbC123/ok.html"), "ok").unwrap();
        assert_eq!(read_guarded(&guard(&alias, "AbC123", "ok.html").unwrap()).unwrap(), "ok");
    }

    #[test]
    fn a_file_swapped_for_a_symlink_after_the_guard_is_not_followed() {
        let (d, root) = setup();
        let f = root.join("AbC123/swap.html");
        fs::write(&f, "x").unwrap();
        let g = guard(&root, "AbC123", "swap.html").unwrap();
        fs::remove_file(&f).unwrap();
        fs::write(d.path().join("t.html"), "x").unwrap();
        std::os::unix::fs::symlink(d.path().join("t.html"), &f).unwrap();
        assert_eq!(read_guarded(&g).unwrap_err(), Rejected::Symlink);
    }

    #[test]
    fn a_fifo_is_rejected_and_never_blocks() {
        let (_d, root) = setup();
        let p = root.join("AbC123/pipe.html");
        let c = std::ffi::CString::new(p.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert_eq!(guard(&root, "AbC123", "pipe.html").unwrap_err(), Rejected::NotAFile);
        // Even if a guarded file became a FIFO, the open does not block and the read is refused.
        let fake = GuardedFile { path: p, size: 0, mtime: 0, dev: 0, ino: 0 };
        assert_eq!(read_guarded(&fake).unwrap_err(), Rejected::NotAFile);
    }

    #[test]
    fn an_id_folder_swapped_for_a_symlink_after_the_guard_is_not_read() {
        let (d, root) = setup();
        let f = root.join("AbC123/same.html");
        fs::write(&f, "real").unwrap();
        let g = guard(&root, "AbC123", "same.html").unwrap();
        // Elsewhere: a file with the same name, size and mtime.
        let other = d.path().join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("same.html"), "evil").unwrap();
        let mtime = fs::metadata(&f).unwrap().modified().unwrap();
        fs::File::options().write(true).open(other.join("same.html")).unwrap().set_modified(mtime).unwrap();
        // Swap the id folder for a symlink to it.
        fs::rename(root.join("AbC123"), d.path().join("moved")).unwrap();
        std::os::unix::fs::symlink(&other, root.join("AbC123")).unwrap();
        assert_eq!(read_guarded(&g).unwrap_err(), Rejected::Changed);
    }

    #[test]
    fn only_html_and_never_tmp() {
        let (_d, root) = setup();
        for name in ["a.htm", "a.txt", "a.html.tmp", "a.tmp", "html", ".html"] {
            fs::write(root.join("AbC123").join(name), "x").unwrap();
            assert_eq!(guard(&root, "AbC123", name).unwrap_err(), Rejected::NotHtml, "{name}");
        }
        fs::create_dir(root.join("AbC123/dir.html")).unwrap();
        assert_eq!(guard(&root, "AbC123", "dir.html").unwrap_err(), Rejected::NotAFile);
    }

    #[test]
    fn five_mib_is_the_limit() {
        let (_d, root) = setup();
        fs::write(root.join("AbC123/ok.html"), vec![b'a'; MAX_HTML_BYTES as usize]).unwrap();
        assert!(read_guarded(&guard(&root, "AbC123", "ok.html").unwrap()).is_ok());
        fs::write(root.join("AbC123/big.html"), vec![b'a'; MAX_HTML_BYTES as usize + 1]).unwrap();
        assert_eq!(guard(&root, "AbC123", "big.html").unwrap_err(), Rejected::TooLarge);
    }

    #[test]
    fn a_size_change_after_the_guard_skips_the_file() {
        let (_d, root) = setup();
        let f = root.join("AbC123/grow.html");
        fs::write(&f, "x").unwrap();
        let g = guard(&root, "AbC123", "grow.html").unwrap();
        fs::write(&f, "xyz").unwrap();
        assert_eq!(read_guarded(&g).unwrap_err(), Rejected::Changed);
        fs::write(&f, vec![b'a'; MAX_HTML_BYTES as usize + 1]).unwrap();
        assert_eq!(read_guarded(&g).unwrap_err(), Rejected::TooLarge);
    }

    #[test]
    fn invalid_utf8_is_read_lossily() {
        let (_d, root) = setup();
        fs::write(root.join("AbC123/bin.html"), [b'<', 0xff, b'>']).unwrap();
        assert_eq!(read_guarded(&guard(&root, "AbC123", "bin.html").unwrap()).unwrap(), "<\u{fffd}>");
    }
}
