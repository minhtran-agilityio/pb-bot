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
//!     token, the app never navigates its main frame, each preview URL works once and only with a nonce
//!     the app minted, every served page starts with a drop guard, and this handler refuses a request
//!     whose `Sec-Fetch-Dest` is present and not `iframe`. The navigation guard (`nav.rs`) blocks every
//!     other URL but cannot tell frames apart (security review L2).
//! Rule: the app has no `message` listener. If one is ever added, its data is untrusted and it never
//! replies with anything derived from IPC.
//! `connect-src 'none'` and `img-src` without hosts also block fetch, XHR, WebSocket, beacons and
//! image loads to any host, claude.ai included.

use super::store;
use rusqlite::Connection;
use std::collections::VecDeque;
use std::io::Read;
use std::sync::Mutex;

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

/// Nonces the app UI minted for the preview frame (`preview_url` command). Each one opens one version
/// once: the scheme refuses a nonce it did not mint, a used one, and one for another version (review F6),
/// so a preview URL that leaks (a drag, a main-frame navigation) loads nothing.
#[derive(Default)]
pub struct PreviewTokens(Mutex<VecDeque<(String, i64)>>);

/// The most nonces kept; older ones expire.
const MAX_TOKENS: usize = 32;

impl PreviewTokens {
    pub fn mint(&self, version_id: i64) -> std::io::Result<String> {
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let nonce: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let mut q = self.0.lock().unwrap_or_else(|p| p.into_inner());
        q.push_back((nonce.clone(), version_id));
        while q.len() > MAX_TOKENS {
            q.pop_front();
        }
        Ok(nonce)
    }

    /// Uses up `nonce` if it was minted for `version_id`.
    pub fn take(&self, nonce: &str, version_id: i64) -> bool {
        let mut q = self.0.lock().unwrap_or_else(|p| p.into_inner());
        match q.iter().position(|(n, v)| n == nonce && *v == version_id) {
            Some(i) => {
                q.remove(i);
                true
            }
            None => false,
        }
    }
}

/// `/v/<n>/<nonce>`: `n` matches `[1-9][0-9]{0,17}` (fits an i64) and the nonce is 32 lower-case hex
/// digits. Anything else is `None`.
pub fn preview_path(path: &str) -> Option<(i64, &str)> {
    let (d, nonce) = path.strip_prefix("/v/")?.split_once('/')?;
    let b = d.as_bytes();
    if b.is_empty() || b.len() > 18 || b[0] == b'0' || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if nonce.len() != 32 || !nonce.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        return None;
    }
    Some((d.parse().ok()?, nonce))
}

/// Runs first in every served page, before any of its own scripts: it cancels the default action of a
/// drop on the frame (opening the dropped file or link), so a drop on the frame does not navigate
/// (review F6). It is registered first, in the capture phase on the window; the page's own handlers run
/// after it and cannot undo `preventDefault`. It does not stop the page from handling drops itself.
const DROP_GUARD: &str = "<script>(function(){function s(e){e.preventDefault()}addEventListener('dragover',s,true);addEventListener('drop',s,true)})()</script>";

/// Puts the meta CSP and the drop guard at the top of the document, after a leading doctype so the
/// page keeps its rendering mode. Compares bytes, so any text is safe (review F1: slicing a `str` at
/// byte 9 panicked inside a multi-byte character).
pub fn with_prelude(html: &str) -> String {
    let prelude = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">{DROP_GUARD}", policy());
    let lead = html.len() - html.trim_start().len();
    let rest = &html.as_bytes()[lead..];
    if rest.get(..9).is_some_and(|b| b.eq_ignore_ascii_case(b"<!doctype")) {
        if let Some(end) = rest.iter().position(|c| *c == b'>') {
            // `>` is ASCII, so the cut is always a char boundary.
            let cut = lead + end + 1;
            return format!("{}{prelude}{}", &html[..cut], &html[cut..]);
        }
    }
    format!("{prelude}{html}")
}

/// Answers one request. Only a GET of `preview://localhost/v/<n>/<nonce>` with a nonce the app minted
/// for that version and not used yet, with no query, from the `main` webview, not as a top-level
/// document (`Sec-Fetch-Dest` absent or `iframe`), for a stored version, returns HTML. Everything else
/// is a 404 with an empty body. It never panics, so it cannot poison the database lock it runs under.
pub fn respond(conn: &Connection, tokens: &PreviewTokens, req: &PreviewRequest) -> PreviewResponse {
    if req.method != "GET" || req.host != Some("localhost") || req.query.is_some() || req.webview_label != "main" {
        return not_found();
    }
    if req.fetch_dest.is_some_and(|d| !d.eq_ignore_ascii_case("iframe")) {
        return not_found();
    }
    let Some((id, nonce)) = preview_path(req.path) else { return not_found() };
    if !tokens.take(nonce, id) {
        return not_found();
    }
    match store::version_html(conn, id) {
        Ok(Some(html)) => {
            let mut headers = security_headers();
            headers.insert(0, ("Content-Type", "text/html; charset=utf-8".into()));
            PreviewResponse { status: 200, headers, body: with_prelude(&html).into_bytes() }
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

    fn url(t: &PreviewTokens, vid: i64) -> String {
        format!("/v/{vid}/{}", t.mint(vid).unwrap())
    }

    #[test]
    fn serves_a_stored_version_once_with_the_strict_csp_and_the_drop_guard() {
        let (_d, c) = conn();
        let t = PreviewTokens::default();
        let vid = stored(&c, "<!DOCTYPE html><html><script>1</script></html>");
        let path = url(&t, vid);
        let r = respond(&c, &t, &get(&path));
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
        // The meta CSP and the drop guard go after the doctype, before the page; no sandbox in the meta.
        let body = String::from_utf8(r.body).unwrap();
        assert!(body.starts_with("<!DOCTYPE html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none';"));
        let head = &body[..body.find("<html>").unwrap()];
        assert!(head.ends_with(DROP_GUARD));
        assert!(!head.contains("sandbox"));
        assert!(body.ends_with("<html><script>1</script></html>"));
        // The same URL a second time: nothing.
        let again = respond(&c, &t, &get(&path));
        assert_eq!((again.status, again.body.len()), (404, 0));
    }

    #[test]
    fn multi_byte_text_at_the_start_never_panics() {
        // Review F1: '<b>ñandú</b>' has a char boundary problem at byte 9.
        for html in ["<b>ñandú</b>", "ñandú", "<!doctypé", "  <!DOCTYPE html>", "Tiếng Việt <!doctype html>", "<!doctype", "", "<!DOCTYPE html>ñ"] {
            let out = with_prelude(html);
            assert!(out.contains(DROP_GUARD), "{html:?}");
            // Nothing of the page is lost: removing the prelude gives the page back.
            let prelude = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">{DROP_GUARD}", policy());
            assert_eq!(out.replacen(&prelude, "", 1), html, "{html:?}");
        }
        let (_d, c) = conn();
        let t = PreviewTokens::default();
        let vid = stored(&c, "<b>ñandú</b>");
        assert_eq!(respond(&c, &t, &get(&url(&t, vid))).status, 200);
    }

    #[test]
    fn prelude_placement() {
        assert!(with_prelude("<p>x</p>").starts_with("<meta http-equiv"));
        let w = with_prelude("\n  <!doctype html>\n<p>");
        assert!(w.starts_with("\n  <!doctype html><meta "), "{w}");
    }

    #[test]
    fn only_a_minted_unused_nonce_for_that_version_opens_it() {
        let (_d, c) = conn();
        let t = PreviewTokens::default();
        let v1 = stored(&c, "<p>1</p>");
        let a = upsert_hook_artifact(&c, &HookMeta { claude_id: "P2", link: None, title: "t", cwd: None, published_at: None }).unwrap();
        let v2 = add_version(&c, a, "<p>2</p>", "hook", None).unwrap().unwrap().0;
        let n = t.mint(v1).unwrap();
        assert_eq!(n.len(), 32);
        assert_eq!(respond(&c, &t, &get(&format!("/v/{v2}/{n}"))).status, 404, "a nonce of another version");
        assert_eq!(respond(&c, &t, &get(&format!("/v/{v1}/{}", "0".repeat(32)))).status, 404, "a nonce nobody minted");
        assert_eq!(respond(&c, &t, &get(&format!("/v/{v1}/{n}"))).status, 200);
        // Old nonces expire.
        let first = t.mint(v1).unwrap();
        for _ in 0..MAX_TOKENS {
            t.mint(v1).unwrap();
        }
        assert!(!t.take(&first, v1));
    }

    #[test]
    fn anything_else_is_an_empty_404() {
        let (_d, c) = conn();
        let t = PreviewTokens::default();
        let vid = stored(&c, "<p>x</p>");
        for p in ["/", "/v/", "/v/abc", "/v/1", "/v/01/x", "/v/1/", "/v/1/../2", "/v/-1/x", "/v/12345678901234567890/x", "/../etc/passwd", "/v/999/x"] {
            let r = respond(&c, &t, &get(p));
            assert_eq!((r.status, r.body.len()), (404, 0), "{p}");
            assert!(header(&r, "Content-Security-Policy").unwrap().contains("sandbox allow-scripts"));
        }
        let path = || url(&t, vid);
        assert_eq!(respond(&c, &t, &PreviewRequest { query: Some("x"), ..get(&path()) }).status, 404, "query");
        assert_eq!(respond(&c, &t, &PreviewRequest { query: Some(""), ..get(&path()) }).status, 404, "empty query");
        assert_eq!(respond(&c, &t, &PreviewRequest { method: "POST", ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { method: "HEAD", ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { host: Some("evil"), ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { host: Some("preview.localhost"), ..get(&path()) }).status, 404, "macOS only");
        assert_eq!(respond(&c, &t, &PreviewRequest { host: None, ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { webview_label: "other", ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { fetch_dest: Some("document"), ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { fetch_dest: Some("script"), ..get(&path()) }).status, 404);
        assert_eq!(respond(&c, &t, &PreviewRequest { fetch_dest: Some("iframe"), ..get(&path()) }).status, 200);
        assert_eq!(respond(&c, &t, &get(&path())).status, 200);
    }

    #[test]
    fn preview_paths() {
        let n = "0123456789abcdef0123456789abcdef";
        assert_eq!(preview_path(&format!("/v/1/{n}")), Some((1, n)));
        assert_eq!(preview_path(&format!("/v/999999999999999999/{n}")).map(|p| p.0), Some(999_999_999_999_999_999));
        assert_eq!(preview_path(&format!("/v/0/{n}")), None);
        assert_eq!(preview_path(&format!("/v/1/{}", n.to_uppercase())), None);
        assert_eq!(preview_path(&format!("/v/1/{n}/x")), None);
        assert_eq!(preview_path("/v/1"), None);
    }
}
