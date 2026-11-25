//! Pod Board's `ui/theme.css` copies the design tokens of Shape Board's `apps/pod-shape/src/styles/tokens.css`.
//! These tests fail when a token the two files share has a different value, in the light or the dark theme.

use std::collections::BTreeMap;
use std::path::PathBuf;

type Tokens = BTreeMap<String, String>;

fn repo_file(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start + 2..].find("*/") {
            Some(end) => &rest[start + 2 + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// The custom properties declared in every rule whose selector is exactly `selector`, merged in file order.
/// Nested rules (a block inside `@media`) count too; the selector is compared with whitespace collapsed.
fn tokens(css: &str, selector: &str) -> Tokens {
    let css = strip_comments(css);
    let want: String = selector.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut found = Tokens::new();
    let mut prelude_start = 0;
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                let sel: String = css[prelude_start..i].split_whitespace().collect::<Vec<_>>().join(" ");
                if sel == want {
                    let end = css[i + 1..].find('}').map(|e| i + 1 + e).unwrap_or(css.len());
                    for decl in css[i + 1..end].split(';') {
                        if let Some((name, value)) = decl.split_once(':') {
                            let name = name.trim();
                            if name.starts_with("--") {
                                found.insert(name.to_string(), normalise(value));
                            }
                        }
                    }
                }
                prelude_start = i + 1;
            }
            b'}' | b';' => prelude_start = i + 1,
            _ => {}
        }
        i += 1;
    }
    found
}

fn normalise(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

const DARK: &str = ":root";
const LIGHT: &str = ":root[data-theme=\"light\"]";
const LIGHT_BY_SYSTEM: &str = ":root:not([data-theme=\"dark\"])";

/// Each token of `shared` that `copy` gives a different value, or leaves out, as one line per token.
fn drift(theme: &str, shared: &Tokens, copy: &Tokens) -> Vec<String> {
    shared
        .iter()
        .filter(|(name, value)| copy.get(*name) != Some(*value))
        .map(|(name, value)| format!("{theme} {name}: tokens.css `{value}`, theme.css `{}`", copy.get(name).map_or("missing", |v| v)))
        .collect()
}

fn theme_drift(tokens_css: &str, theme_css: &str) -> Vec<String> {
    let mut out = drift("dark", &tokens(tokens_css, DARK), &tokens(theme_css, DARK));
    out.extend(drift("light", &tokens(tokens_css, LIGHT), &tokens(theme_css, LIGHT)));
    out.extend(drift("light (system)", &tokens(tokens_css, LIGHT_BY_SYSTEM), &tokens(theme_css, LIGHT_BY_SYSTEM)));
    out
}

#[test]
fn theme_css_shares_every_token_value_of_shape_board() {
    let shape = repo_file("../../pod-shape/src/styles/tokens.css");
    let pod = repo_file("../ui/theme.css");
    assert!(tokens(&shape, DARK).len() > 30, "tokens.css should declare the dark token set");
    assert!(tokens(&shape, LIGHT).len() > 20, "tokens.css should declare the light token set");
    let diffs = theme_drift(&shape, &pod);
    assert!(diffs.is_empty(), "ui/theme.css differs from tokens.css:\n{}", diffs.join("\n"));
}

#[test]
fn a_changed_value_in_either_theme_is_reported() {
    let shape = repo_file("../../pod-shape/src/styles/tokens.css");
    let pod = repo_file("../ui/theme.css");
    // Dark: the default `:root` block holds `--bg: #211A15`.
    let dark = pod.replacen("--bg:             #211A15", "--bg:             #000000", 1);
    assert_ne!(dark, pod, "the dark --bg line was not found in theme.css");
    assert!(theme_drift(&shape, &dark).iter().any(|d| d.starts_with("dark --bg:")));
    // Light: the `data-theme="light"` block.
    let at = pod.find(LIGHT).expect("theme.css has a light block");
    let light = format!("{}{}", &pod[..at], pod[at..].replacen("--accent: #A9461F", "--accent: #C4552B", 1));
    assert_ne!(light, pod, "the light --accent line was not found in theme.css");
    assert!(theme_drift(&shape, &light).iter().any(|d| d.starts_with("light --accent:")));
    // A token that disappears from the copy is reported too.
    let gone = pod.replacen("--ring:           #EB7D4C;", "", 1);
    assert!(theme_drift(&shape, &gone).iter().any(|d| d.starts_with("dark --ring:") && d.ends_with("`missing`")));
}

#[test]
fn the_light_theme_is_the_same_whether_chosen_or_from_the_system() {
    for (name, rel) in [("tokens.css", "../../pod-shape/src/styles/tokens.css"), ("theme.css", "../ui/theme.css")] {
        let css = repo_file(rel);
        let chosen = tokens(&css, LIGHT);
        let system = tokens(&css, LIGHT_BY_SYSTEM);
        assert_eq!(chosen, system, "{name}: the two light blocks differ");
    }
}
