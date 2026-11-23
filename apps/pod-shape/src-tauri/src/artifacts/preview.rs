//! The `preview://` scheme: serves one stored version's HTML to the sandboxed preview iframe with its
//! own CSP (HANDOFF §3 "Prototype preview"). It does not use srcdoc, because a srcdoc frame inherits
//! the app CSP (`script-src 'self'`), which blocks a prototype's inline scripts.
//!
//! Why the page cannot reach Tauri commands (design.md §8, checked in tauri 2.12.7 after the advisor
//! review). Tauri treats every registered scheme as a local origin, so capabilities do NOT scope this
//! frame. The barriers are:
//! (a) the invoke key never enters the frame: Tauri injects the IPC bootstrap and the key into the
//!     main frame only, and rejects any request without the key;
//! (b) untrusted HTML never becomes a main-frame document: the frame's sandbox has no top-navigation
//!     token, the app never navigates its main frame, and this handler refuses a request whose
//!     `Sec-Fetch-Dest` is present and not `iframe`. The navigation guard (`nav.rs`) blocks every other
//!     URL but cannot tell frames apart (security review L2).
//! Rule: the app has no `message` listener. If one is ever added, its data is untrusted and it never
//! replies with anything derived from IPC.
//! `connect-src 'none'` and `img-src` without hosts also block fetch, XHR, WebSocket, beacons and
//! image loads to any host, claude.ai included.

use super::store;
use rusqlite::Connection;

/// Script, style and font hosts a published artifact may load from (owner decision 2026-11-23,
/// option A: the Artifact tool's five CDN hosts and Google Fonts; exact hosts, no wildcards;
/// jsdelivr only under `/npm/`).
const CDN: &str = "https://cdnjs.cloudflare.com https://cdn.jsdelivr.net/npm/ https://unpkg.com https://cdn.tailwindcss.com https://code.jquery.com";

/// The policy without the `sandbox` directive (a `<meta>` CSP cannot carry `sandbox`).
pub fn policy() -> String {
    format!(
        "default-src 'none'; \
         script-src 'unsafe-inline' 'unsafe-eval' data: blob: {CDN}; \
         style-src 'unsafe-inline' data: {CDN} https://fonts.googleapis.com; \
         font-src data: {CDN} https://fonts.gstatic.com; \
         img-src data: blob:; media-src data: blob:; \
         connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src blob:; \
         form-action 'none'; base-uri 'none'; object-src 'none'"
    )
}

/// The header policy: `policy()` plus `sandbox allow-scripts`, the same sandbox as the iframe.
pub fn header_csp() -> String {
    format!("{}; sandbox allow-scripts", policy())
}

#[derive(Debug, PartialEq)]
pub struct PreviewResponse {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// One request, reduced to what the handler checks.
pub struct PreviewRequest<'a> {
    pub method: &'a str,
    pub host: Option<&'a str>,
    pub path: &'a str,
    pub query: Option<&'a str>,
    pub webview_label: &'a str,
    /// The `Sec-Fetch-Dest` header, when the engine sends one.
    pub fetch_dest: Option<&'a str>,
}

fn security_headers() -> Vec<(&'static str, String)> {
    vec![
        ("Content-Security-Policy", header_csp()),
        ("X-Content-Type-Options", "nosniff".into()),
        ("Referrer-Policy", "no-referrer".into()),
        ("Cache-Control", "no-store".into()),
    ]
}

pub fn not_found() -> PreviewResponse {
    PreviewResponse { status: 404, headers: security_headers(), body: Vec::new() }
}

/// The version id in `/v/<n>`: `n` matches `[1-9][0-9]{0,17}`, so it fits an i64. Anything else is `None`.
pub fn version_id(path: &str) -> Option<i64> {
    let d = path.strip_prefix("/v/")?;
    let b = d.as_bytes();
    if b.is_empty() || b.len() > 18 || b[0] == b'0' || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    d.parse().ok()
}

/// Puts the meta CSP at the top of the document, after a leading doctype so the page keeps its
/// rendering mode.
pub fn with_meta_csp(html: &str) -> String {
    let meta = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">", policy());
    let trimmed = html.trim_start();
    let lead = html.len() - trimmed.len();
    if trimmed.len() >= 9 && trimmed[..9].eq_ignore_ascii_case("<!doctype") {
        if let Some(end) = trimmed.find('>') {
            let cut = lead + end + 1;
            return format!("{}{meta}{}", &html[..cut], &html[cut..]);
        }
    }
    format!("{meta}{html}")
}

/// Answers one request. Only GET or HEAD of `preview://localhost/v/<n>` (or, on Windows only, the form
/// `http://preview.localhost/v/<n>`), with no query, from the `main` webview, not as a top-level
/// document (`Sec-Fetch-Dest` absent or `iframe`), for a stored version, returns HTML. Everything else
/// is a 404 with an empty body.
pub fn respond(conn: &Connection, req: &PreviewRequest) -> PreviewResponse {
    let head = match req.method {
        "GET" => false,
        "HEAD" => true,
        _ => return not_found(),
    };
    let host_ok = req.host == Some("localhost") || (cfg!(windows) && req.host == Some("preview.localhost"));
    if !host_ok || req.query.is_some() || req.webview_label != "main" {
        return not_found();
    }
    if req.fetch_dest.is_some_and(|d| !d.eq_ignore_ascii_case("iframe")) {
        return not_found();
    }
    let Some(id) = version_id(req.path) else { return not_found() };
    match store::version_html(conn, id) {
        Ok(Some(html)) => {
            let mut headers = security_headers();
            headers.insert(0, ("Content-Type", "text/html; charset=utf-8".into()));
            let body = if head { Vec::new() } else { with_meta_csp(&html).into_bytes() };
            PreviewResponse { status: 200, headers, body }
        }
        _ => not_found(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::store::{add_version, tests::conn, upsert_hook_artifact, HookMeta};

    fn header<'a>(r: &'a PreviewResponse, k: &str) -> Option<&'a str> {
        r.headers.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
    }

    fn get(path: &str) -> PreviewRequest<'_> {
        PreviewRequest { method: "GET", host: Some("localhost"), path, query: None, webview_label: "main", fetch_dest: None }
    }

    fn stored(c: &Connection, html: &str) -> i64 {
        let a = upsert_hook_artifact(c, &HookMeta { claude_id: "P1", link: None, title: "t", cwd: None, published_at: None }).unwrap();
        add_version(c, a, html, "hook", None).unwrap().unwrap().0
    }

    #[test]
    fn serves_a_stored_version_with_the_strict_csp() {
        let (_d, c) = conn();
        let vid = stored(&c, "<!DOCTYPE html><html><script>1</script></html>");
        let r = respond(&c, &get(&format!("/v/{vid}")));
        assert_eq!(r.status, 200);
        assert_eq!(header(&r, "Content-Type"), Some("text/html; charset=utf-8"));
        let csp = header(&r, "Content-Security-Policy").unwrap();
        for must in [
            "default-src 'none'",
            "connect-src 'none'",
            "frame-src 'none'",
            "form-action 'none'",
            "base-uri 'none'",
            "img-src data: blob:;",
            "https://cdn.jsdelivr.net/npm/",
            "sandbox allow-scripts",
        ] {
            assert!(csp.contains(must), "{must}");
        }
        assert!(csp.ends_with("; sandbox allow-scripts"));
        for never in ["claude.ai", "*", "'self'", "allow-same-origin", "allow-top-navigation", "allow-popups", "allow-forms"] {
            assert!(!csp.contains(never), "{never}");
        }
        assert_eq!(header(&r, "X-Content-Type-Options"), Some("nosniff"));
        assert_eq!(header(&r, "Referrer-Policy"), Some("no-referrer"));
        assert_eq!(header(&r, "Cache-Control"), Some("no-store"));
        assert!(r.headers.iter().all(|(k, _)| !k.starts_with("Access-Control")));
        // The meta CSP goes after the doctype and has no sandbox directive.
        let body = String::from_utf8(r.body).unwrap();
        assert!(body.starts_with("<!DOCTYPE html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none';"));
        assert!(body.ends_with("<html><script>1</script></html>"));
        assert!(!body[..body.find("<html>").unwrap()].contains("sandbox"));
    }

    #[test]
    fn meta_csp_placement() {
        assert!(with_meta_csp("<p>x</p>").starts_with("<meta http-equiv"));
        let w = with_meta_csp("\n  <!doctype html>\n<p>");
        assert!(w.starts_with("\n  <!doctype html><meta "), "{w}");
    }

    #[test]
    fn head_has_headers_and_no_body() {
        let (_d, c) = conn();
        let vid = stored(&c, "<p>x</p>");
        let path = format!("/v/{vid}");
        let r = respond(&c, &PreviewRequest { method: "HEAD", ..get(&path) });
        assert_eq!((r.status, r.body.len()), (200, 0));
    }

    #[test]
    fn anything_else_is_an_empty_404() {
        let (_d, c) = conn();
        let vid = stored(&c, "<p>x</p>");
        let good = format!("/v/{vid}");
        for p in ["/", "/v/", "/v/abc", "/v/01", "/v/1/", "/v/1/../2", "/v/-1", "/v/+1", "/v/12345678901234567890", "/../etc/passwd", "/v/999"] {
            let r = respond(&c, &get(p));
            assert_eq!((r.status, r.body.len()), (404, 0), "{p}");
            assert!(header(&r, "Content-Security-Policy").unwrap().contains("sandbox allow-scripts"));
        }
        assert_eq!(respond(&c, &PreviewRequest { query: Some("x"), ..get(&good) }).status, 404, "query");
        assert_eq!(respond(&c, &PreviewRequest { query: Some(""), ..get(&good) }).status, 404, "empty query");
        assert_eq!(respond(&c, &PreviewRequest { method: "POST", ..get(&good) }).status, 404);
        assert_eq!(respond(&c, &PreviewRequest { host: Some("evil"), ..get(&good) }).status, 404);
        assert_eq!(respond(&c, &PreviewRequest { host: None, ..get(&good) }).status, 404);
        assert_eq!(respond(&c, &PreviewRequest { webview_label: "other", ..get(&good) }).status, 404);
        assert_eq!(respond(&c, &get(&good)).status, 200);
        let win = respond(&c, &PreviewRequest { host: Some("preview.localhost"), ..get(&good) }).status;
        assert_eq!(win, if cfg!(windows) { 200 } else { 404 }, "the Windows host form only on Windows");
        // Only a frame load: a top-level document request is refused.
        assert_eq!(respond(&c, &PreviewRequest { fetch_dest: Some("iframe"), ..get(&good) }).status, 200);
        assert_eq!(respond(&c, &PreviewRequest { fetch_dest: Some("document"), ..get(&good) }).status, 404);
        assert_eq!(respond(&c, &PreviewRequest { fetch_dest: Some("script"), ..get(&good) }).status, 404);
    }

    #[test]
    fn version_ids() {
        assert_eq!(version_id("/v/1"), Some(1));
        assert_eq!(version_id("/v/999999999999999999"), Some(999_999_999_999_999_999));
        assert_eq!(version_id("/v/0"), None);
        assert_eq!(version_id("/v/1x"), None);
    }
}
