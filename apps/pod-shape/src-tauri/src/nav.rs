//! Navigation guard of the main window (design.md §8, advisor item 2). It allows only the app itself,
//! the preview scheme and `about:blank` / `about:srcdoc`; everything else (`file:`, `data:`, `https:` ...)
//! is denied, in the main frame and (on WebKit) in the preview frame. New windows and downloads are
//! denied in `lib.rs`.
//!
//! Limit (security review L2): the callback gets only the URL, not the frame, so it cannot refuse
//! `preview://localhost/v/<n>` in the main frame. What keeps untrusted HTML out of the main frame (which
//! holds the IPC invoke key) is: the frame's sandbox has no top-navigation token, the app never
//! navigates its main frame, drops cannot navigate (`main.tsx` and the drop guard in every served page),
//! and the preview scheme answers each minted URL once and only a request whose `Sec-Fetch-Dest`, when
//! the engine sends one, is `iframe` (`artifacts/preview.rs`). The app is macOS only (owner decision),
//! so the Windows `http(s)://*.localhost` forms are not allowed.

use crate::artifacts::preview::preview_path;
use tauri::Url;

/// True when `url` may load in the main window (top frame or, where the engine asks, a subframe).
/// `dev_url` is the Vite dev server, passed only in debug builds.
pub fn allowed(url: &Url, dev_url: Option<&Url>) -> bool {
    let host = url.host_str();
    match url.scheme() {
        "about" => matches!(url.path(), "blank" | "srcdoc"),
        // The app: tauri://localhost on macOS (and Linux, where it is tested).
        "tauri" => host == Some("localhost"),
        // One stored version in the preview frame (`/v/<id>/<nonce>`); no query, so nothing rides along.
        "preview" => host == Some("localhost") && url.query().is_none() && url.port().is_none() && preview_path(url.path()).is_some(),
        _ => dev_url.is_some_and(|d| d.origin() == url.origin()),
    }
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
            "preview://localhost/v/1/0123456789abcdef0123456789abcdef",
            "preview://localhost/v/42/0123456789abcdef0123456789abcdef",
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
            "preview://localhost/v/1",
            "preview://localhost/v/01/0123456789abcdef0123456789abcdef",
            "preview://localhost/v/1/0123456789abcdef0123456789abcdef?x=1",
            "preview://evil/v/1/0123456789abcdef0123456789abcdef",
            "preview://localhost:99/v/1/0123456789abcdef0123456789abcdef",
            "http://preview.localhost/v/1/0123456789abcdef0123456789abcdef",
            "http://tauri.localhost/",
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
