//! Navigation guard of the main window (design.md §8, advisor item 2). It allows only the app itself,
//! the preview scheme and `about:blank` / `about:srcdoc`; everything else (`file:`, `data:`, `https:` ...)
//! is denied, in the main frame and (on WebKit) in the preview frame. New windows and downloads are
//! denied in `lib.rs`.
//!
//! Limit (security review L2): the callback gets only the URL, not the frame, so it cannot refuse
//! `preview://localhost/v/<n>` in the main frame. What keeps untrusted HTML out of the main frame (which
//! holds the IPC invoke key) is: the frame's sandbox has no top-navigation token, the app never
//! navigates its main frame, drops cannot navigate (`main.tsx`), and the preview scheme answers only a
//! request whose `Sec-Fetch-Dest`, when the engine sends one, is `iframe` (`artifacts/preview.rs`).

use crate::artifacts::preview::version_id;
use tauri::Url;

/// True when `url` may load in the main window (top frame or, where the engine asks, a subframe).
/// `dev_url` is the Vite dev server, passed only in debug builds.
pub fn allowed(url: &Url, dev_url: Option<&Url>) -> bool {
    let host = url.host_str();
    match url.scheme() {
        "about" => matches!(url.path(), "blank" | "srcdoc"),
        // The app: tauri://localhost on macOS and Linux. The http(s)://*.localhost forms are Windows-only
        // (security review L3): elsewhere they could be a real local server.
        "tauri" => host == Some("localhost"),
        "http" | "https" if cfg!(windows) && host == Some("tauri.localhost") => true,
        // One stored version in the preview frame; no query, so nothing extra rides along.
        "preview" => host == Some("localhost") && preview_path(url),
        "http" if cfg!(windows) && host == Some("preview.localhost") => preview_path(url),
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
            "preview://localhost/v/1",
            "preview://localhost/v/42",
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
    fn windows_only_origins_follow_the_platform() {
        for u in ["http://tauri.localhost/", "https://tauri.localhost/", "http://preview.localhost/v/7"] {
            assert_eq!(ok(u), cfg!(windows), "{u}");
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
