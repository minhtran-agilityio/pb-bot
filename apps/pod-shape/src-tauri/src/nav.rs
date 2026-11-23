//! Navigation guard of the main window (design.md §8, advisor item 2). Untrusted HTML must never become
//! a main-frame document, because the main frame holds the IPC invoke key. The window allows only the
//! app itself, the preview scheme and `about:blank` / `about:srcdoc`; everything else (`file:`,
//! `data:`, `https:` ...) is denied. New windows and downloads are denied in `lib.rs`.

use crate::artifacts::preview::version_id;
use tauri::Url;

/// True when `url` may load in the main window (top frame or, where the engine asks, a subframe).
/// `dev_url` is the Vite dev server, passed only in debug builds.
pub fn allowed(url: &Url, dev_url: Option<&Url>) -> bool {
    let host = url.host_str();
    match url.scheme() {
        "about" => matches!(url.path(), "blank" | "srcdoc"),
        // The app: tauri://localhost on macOS and Linux, http(s)://tauri.localhost on Windows.
        "tauri" => host == Some("localhost"),
        "http" | "https" if host == Some("tauri.localhost") => true,
        // One stored version in the preview frame; no query, so nothing extra rides along.
        "preview" => host == Some("localhost") && preview_path(url),
        "http" if host == Some("preview.localhost") => preview_path(url),
        _ => dev_url.is_some_and(|d| d.origin() == url.origin()),
    }
}

fn preview_path(url: &Url) -> bool {
    url.query().is_none() && url.port().is_none() && version_id(url.path()).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(u: &str) -> bool {
        allowed(&Url::parse(u).unwrap(), None)
    }

    #[test]
    fn the_app_and_one_preview_version_are_allowed() {
        for u in [
            "tauri://localhost",
            "tauri://localhost/index.html",
            "http://tauri.localhost/",
            "preview://localhost/v/1",
            "preview://localhost/v/42",
            "http://preview.localhost/v/7",
            "about:blank",
            "about:srcdoc",
        ] {
            assert!(ok(u), "{u}");
        }
    }

    #[test]
    fn everything_else_is_denied() {
        for u in [
            "file:///etc/passwd",
            "file:///Users/m/claude-artifacts/x/a.html",
            "data:text/html,<script>1</script>",
            "https://example.com/",
            "https://claude.ai/artifact/x",
            "http://localhost:1430/",
            "javascript:alert(1)",
            "blob:tauri://localhost/123",
            "preview://localhost/",
            "preview://localhost/v/01",
            "preview://localhost/v/1?x=1",
            "preview://evil/v/1",
            "preview://localhost:99/v/1",
            "http://preview.localhost/v/1?x",
            "tauri://evil/",
            "about:config",
            "ipc://localhost/list_projects",
        ] {
            assert!(!ok(u), "{u}");
        }
    }

    #[test]
    fn the_dev_server_only_when_given() {
        let dev = Url::parse("http://localhost:1430").unwrap();
        assert!(allowed(&Url::parse("http://localhost:1430/").unwrap(), Some(&dev)));
        assert!(allowed(&Url::parse("http://localhost:1430/src/main.tsx").unwrap(), Some(&dev)));
        assert!(!allowed(&Url::parse("http://localhost:1431/").unwrap(), Some(&dev)));
        assert!(!allowed(&Url::parse("http://localhost:1430/").unwrap(), None));
    }
}
