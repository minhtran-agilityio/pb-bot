"""Acceptance tests for GitHub issue #136: Pod Board adopts the UI design system (UDS v3.0).

Run: python3 apps/pod-board/tests/acceptance/test_uds_adoption.py   (or: bash apps/pod-board/tests/acceptance/run.sh)

Outside-in and headless: static checks over the shipped HTML/CSS/config files, and a headless Chrome that loads
copies of the two pages (with no Tauri runtime and no data) and reads computed styles. Nothing in the app is changed.
Chrome tests are skipped, loudly, when no Chrome/Chromium binary exists.
Criteria that need macOS or live session data are listed in README.md as "manual on M0".
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from html.parser import HTMLParser
from pathlib import Path

HERE = Path(__file__).resolve().parent
POD_BOARD = HERE.parent.parent
ROOT = POD_BOARD.parent.parent
UI = POD_BOARD / "ui"
TOKENS_CSS = ROOT / "apps/pod-shape/src/styles/tokens.css"
CHROME = next((p for p in (shutil.which(n) for n in ("google-chrome", "chromium", "chromium-browser", "google-chrome-stable")) if p), None)

# Names the ticket (Decisions, point 1) says theme.css adds on top of the shared token set.
POD_TOKENS = [
    "--session-needs", "--session-needs-soft", "--session-working", "--session-working-soft",
    "--session-idle", "--session-idle-soft", "--session-finished", "--session-finished-soft",
    "--pressed-sh", "--pod-brand-bg", "--pod-brand-cat", "--pod-brand-eye",
]

_tmp_dirs = []


def tmpdir():
    d = tempfile.mkdtemp(prefix="pod-board-acc-")
    _tmp_dirs.append(d)
    return Path(d)


def tearDownModule():
    for d in _tmp_dirs:
        shutil.rmtree(d, ignore_errors=True)


def read(p):
    return Path(p).read_text(encoding="utf-8")


def strip_comments(css):
    return re.sub(r"/\*.*?\*/", "", css, flags=re.S)


# ---------- Chrome helpers ----------

def run_chrome(html_path, extra=()):
    prof = tmpdir()
    cmd = [CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--allow-file-access-from-files",
           "--window-size=1180,720", "--virtual-time-budget=10000", f"--user-data-dir={prof}",
           "--dump-dom", *extra, Path(html_path).as_uri()]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=90)
    m = re.search(r'<pre id="__probe">(.*?)</pre>', r.stdout, flags=re.S)
    if not m:
        raise AssertionError(f"probe produced no result (chrome exit {r.returncode}): {r.stderr[-400:]}")
    import html
    return json.loads(html.unescape(m.group(1)))


_page_cache = {}


def probe_page(name, mode):
    """Copy ui/ to a temp dir, add the probe script before </body>, run Chrome, return the JSON."""
    key = (name, mode)
    if key in _page_cache:
        return _page_cache[key]
    d = tmpdir() / "ui"
    shutil.copytree(UI, d)
    page = d / name
    src = read(page)
    probe = read(HERE / "probe.js")
    inject = f"<script>window.__PROBE_MODE={json.dumps(mode)};</script><script>{probe}</script>"
    src = src.replace("</body>", inject + "</body>") if "</body>" in src else src + inject
    page.write_text(src, encoding="utf-8")
    _page_cache[key] = run_chrome(page)
    return _page_cache[key]


def css_names(path):
    """Custom properties declared in :root blocks of a stylesheet."""
    css = strip_comments(read(path))
    names, i, stack = [], 0, []
    for m in re.finditer(r"([^{}]*)\{|\}", css):
        if m.group(0) == "}":
            if stack:
                stack.pop()
            continue
        sel = m.group(1).strip()
        stack.append(sel)
        if sel.startswith(":root"):
            end = css.find("}", m.end())
            names += re.findall(r"(--[\w-]+)\s*:", css[m.end():end])
    return sorted(set(names))


def token_values(css_file, names):
    d = tmpdir()
    shutil.copy(css_file, d / "t.css")
    (d / "p.html").write_text(
        '<!doctype html><html><head><link rel="stylesheet" href="t.css"></head><body>'
        f"<script>window.__NAMES={json.dumps(names)};</script><script>{read(HERE / 'tokenprobe.js')}</script></body></html>")
    return run_chrome(d / "p.html")


def norm(v):
    v = v.strip().lower()
    v = re.sub(r"\s+", " ", v)
    v = re.sub(r"\s*([,()])\s*", r"\1", v)
    v = re.sub(r"(?<![\d])0\.", ".", v)
    return v


def to_rgb(v):
    v = v.strip()
    m = re.fullmatch(r"#([0-9a-fA-F]{6})", v)
    if m:
        h = m.group(1)
        return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), 1.0)
    m = re.fullmatch(r"rgba?\(([^)]*)\)", v)
    if m:
        p = [x.strip() for x in m.group(1).replace("/", ",").split(",")]
        return (float(p[0]), float(p[1]), float(p[2]), float(p[3]) if len(p) > 3 else 1.0)
    raise ValueError(f"not a colour: {v!r}")


def over(fg, bg):
    a = fg[3]
    return tuple(fg[i] * a + bg[i] * (1 - a) for i in range(3)) + (1.0,)


def lum(c):
    def ch(x):
        x /= 255
        return x / 12.92 if x <= 0.03928 else ((x + 0.055) / 1.055) ** 2.4
    return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def hex_to_css_rgb(h):
    r, g, b, _ = to_rgb(h)
    return f"rgb({int(r)}, {int(g)}, {int(b)})"


needs_chrome = unittest.skipUnless(CHROME, "no google-chrome/chromium on this machine: headless page checks skipped")


class SvgScan(HTMLParser):
    """Collect text of the HTML outside of <svg> elements that belong to the logo or the cat (and <style>/<script> are kept)."""
    def __init__(self):
        super().__init__(convert_charrefs=False)
        self.stack = []        # (tag, is_art)
        self.svg_depth = 0
        self.art_depth = 0
        self.chunks = []       # text outside art

    def _art(self, attrs):
        a = " ".join(v or "" for k, v in attrs if k in ("id", "class")).lower()
        return bool(re.search(r"cat|logo|pod-brand|mascot", a))

    def handle_starttag(self, tag, attrs):
        art = self._art(attrs) or self.art_depth > 0
        if tag in ("svg", "symbol", "g", "path", "circle", "ellipse", "rect", "defs", "use", "line", "polygon", "polyline"):
            pass
        text = self.get_starttag_text()
        if tag in ("svg", "symbol") or self.art_depth == 0:
            pass
        if self.art_depth == 0 and not (tag in ("svg", "symbol") and art):
            self.chunks.append(text)
        if art and tag in ("svg", "symbol", "div", "span", "i", "b"):
            self.art_depth += 1
            self.stack.append((tag, True))
        elif self.art_depth > 0:
            self.stack.append((tag, False))
        else:
            self.stack.append((tag, False))

    def handle_startendtag(self, tag, attrs):
        if self.art_depth == 0:
            self.chunks.append(self.get_starttag_text())

    def handle_endtag(self, tag):
        while self.stack:
            t, art = self.stack.pop()
            if art:
                self.art_depth -= 1
            if t == tag:
                break

    def handle_data(self, data):
        if self.art_depth == 0:
            self.chunks.append(data)


# ============================================================================
class TestTokens(unittest.TestCase):
    # AC1: "`cargo test` in `apps/pod-board/src-tauri` passes, with one new test that fails when a token shared by
    # `ui/theme.css` and `apps/pod-shape/src/styles/tokens.css` has a different value (light or dark)."
    # The cargo part is for the implementer to add and run; this test is the same drift check from the outside.
    @needs_chrome
    def test_AC1_shared_tokens_equal_in_light_and_dark(self):
        names = css_names(TOKENS_CSS)
        self.assertGreater(len(names), 30, "tokens.css should declare the UDS token set")
        a = token_values(TOKENS_CSS, names)
        b = token_values(UI / "theme.css", names)
        diffs = []
        for theme in ("light", "dark"):
            for n in names:
                if norm(a[theme][n]) != norm(b[theme][n]):
                    diffs.append(f"{theme} {n}: tokens.css={a[theme][n]!r} theme.css={b[theme][n]!r}")
        self.assertEqual(diffs, [], "theme.css differs from tokens.css:\n" + "\n".join(diffs[:20]))

    # Decision 1: "plus the Pod Board tokens of UDS v3.0: `--session-needs`, `--session-working`, `--session-idle`,
    # `--session-finished` with their `-soft` pairs, `--pressed-sh`, and `--pod-brand-bg/cat/eye` for the logo only."
    @needs_chrome
    def test_AC1_pod_board_tokens_are_defined(self):
        v = token_values(UI / "theme.css", POD_TOKENS)
        missing = [f"{t} {n}" for t in ("light", "dark") for n in POD_TOKENS if not v[t][n]]
        self.assertEqual(missing, [])

    # AC15 (text contrast part): "Every text pair passes AA 4.5:1 in both themes (check the muted meta text, the
    # tags and the pills)."  Pairs: the text tokens on every surface, the tag/pill text on its soft fill.
    @needs_chrome
    def test_AC14_text_pairs_pass_AA_in_both_themes(self):
        extra = ["--session-needs-soft", "--session-working", "--session-working-soft", "--session-idle", "--session-idle-soft",
                 "--session-finished", "--session-finished-soft"]
        names = ["--bg", "--surface-1", "--surface-2", "--surface-3", "--side", "--fg", "--fg-muted", "--fg-subtle",
                 "--accent-text", "--accent", "--accent-fg", "--success", "--status-open-soft", "--danger", "--danger-soft",
                 "--warning", "--warning-soft"] + extra
        v = token_values(UI / "theme.css", names)
        fails = []
        for theme in ("light", "dark"):
            c, bad = {}, []
            for n in names:
                try:
                    c[n] = to_rgb(v[theme][n])
                except ValueError:
                    bad.append(f"{n}={v[theme][n]!r}")
            self.assertEqual(bad, [], f"{theme}: tokens missing or not a #hex/rgb() colour")
            pairs = []
            for t in ("--fg", "--fg-muted", "--fg-subtle"):
                for s in ("--bg", "--surface-1", "--surface-2", "--side"):
                    pairs.append((t, s, c[t], c[s]))
            pairs.append(("--accent-fg", "--accent", c["--accent-fg"], c["--accent"]))
            for surf in ("--bg", "--surface-2"):
                base = c[surf]
                for fg, soft in (("--accent-text", "--session-needs-soft"), ("--session-working", "--session-working-soft"),
                                 ("--session-idle", "--session-idle-soft"), ("--session-finished", "--session-finished-soft"),
                                 ("--success", "--status-open-soft"), ("--fg", "--danger-soft"), ("--warning", "--warning-soft")):
                    fill = over(c[soft], base)
                    pairs.append((fg, f"{soft} on {surf}", c[fg], fill))
                pairs.append(("--accent-text", surf, c["--accent-text"], base))
            # the app draws --danger text on plain --surface-2 (peek error), not on --danger-soft; the banner uses --fg on --danger-soft
            pairs.append(("--danger", "--surface-2", c["--danger"], c["--surface-2"]))
            for fg, bg, a, b in pairs:
                a = over(a, b) if a[3] < 1 else a
                r = contrast(a, b)
                if r < 4.5:
                    fails.append(f"{theme}: {fg} on {bg} = {r:.2f}")
        self.assertEqual(fails, [], "\n".join(fails))


class TestStaticFiles(unittest.TestCase):
    # AC2: "`grep -nE "#[0-9a-fA-F]{3,8}|hsl\(|rgba?\(" apps/pod-board/ui/index.html apps/pod-board/ui/popover.html`
    # returns only the logo and cat drawings. Colours come from `theme.css`."
    def test_AC2_no_raw_colours_outside_logo_and_cat_drawings(self):
        pat = re.compile(r"#[0-9a-fA-F]{3,8}\b|hsl\(|rgba?\(")
        offenders = []
        for name in ("index.html", "popover.html"):
            p = SvgScan()
            p.feed(read(UI / name))
            text = "".join(p.chunks)
            # a '#' that opens a CSS id selector or an href to a sprite symbol is not a colour
            for m in pat.finditer(text):
                before = text[max(0, m.start() - 1):m.start()]
                s = m.group(0)
                if s.startswith("#") and (before in ("\"", "'") and False):
                    continue
                if s.startswith("#") and re.match(r"#[0-9a-fA-F]{3,8}\b", s) and text[max(0, m.start() - 6):m.start()].endswith(("href=\"", "href='", "url(")):
                    continue
                offenders.append(f"{name}: {text[max(0, m.start() - 40):m.end() + 20]!r}")
        self.assertEqual(offenders, [], "raw colours outside the logo and cat drawings:\n" + "\n".join(offenders[:15]))

    # AC3: "`grep -rn "uppercase\|backdrop-filter\|grain\|Ubuntu\|DejaVu\|font-weight: [78]00" apps/pod-board/ui` returns nothing."
    def test_AC3_banned_patterns_absent_from_ui(self):
        r = subprocess.run(["grep", "-rn", r"uppercase\|backdrop-filter\|grain\|Ubuntu\|DejaVu\|font-weight: [78]00", str(UI)],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 1, f"grep found matches:\n{r.stdout[:800]}")
        self.assertEqual(r.stdout, "")

    # Decision 14: "Removed: grain, blur, uppercase labels, weights above 600, tinted lanes, the peach summary."
    def test_AC3_no_blur_filter_or_weight_above_600_in_css(self):
        text = "".join(read(p) for p in UI.glob("*") if p.suffix in (".html", ".css"))
        hits = re.findall(r"blur\(|font-weight\s*:\s*(?:[789]00|bold|bolder)", strip_comments(text))
        self.assertEqual(hits, [])

    # AC4: "With the network off, the window and the popover show Outfit text ... No request leaves the app (the CSP is unchanged)."
    def test_AC4_fonts_are_bundled_locally(self):
        css = strip_comments(read(UI / "theme.css"))
        faces = re.findall(r"@font-face\s*\{([^}]*)\}", css)
        fam = {}
        for f in faces:
            name = re.search(r"font-family\s*:\s*['\"]?([^;'\"]+)", f).group(1).strip()
            weight = re.search(r"font-weight\s*:\s*([\d ]+)", f)
            urls = re.findall(r"url\(['\"]?([^)'\"]+)", f)
            fam.setdefault(name, []).append((weight.group(1).strip() if weight else "400", urls))
        self.assertIn("Outfit", fam, "theme.css must declare @font-face for Outfit")
        self.assertIn("JetBrains Mono", fam, "theme.css must declare @font-face for JetBrains Mono")
        for w in ("400", "500", "600"):
            self.assertTrue(any(x[0] == w for x in fam["Outfit"]) or any(" " in x[0] for x in fam["Outfit"]), f"Outfit {w} missing")
        for _, urls in fam["Outfit"] + fam["JetBrains Mono"]:
            self.assertTrue(urls)
            for u in urls:
                self.assertFalse(re.match(r"(https?:)?//", u), f"font loads from the network: {u}")
                self.assertTrue((UI / u).is_file(), f"font file missing: {u}")
        files = [p.name for p in (UI / "fonts").iterdir()]
        self.assertEqual([f for f in files if re.search(r"ubuntu|dejavu", f, re.I)], [], "old fonts still bundled")

    # AC5: "title bar with Pod Cat logo ... Dragging the title bar moves the window." Decision 4: "the macOS window
    # buttons sit on it; it drags the window" (Agent notes: `titleBarStyle: "Overlay"`, `hiddenTitle: true`,
    # `data-tauri-drag-region`, and the permission `core:window:allow-start-dragging` in `capabilities/default.json`).
    def test_AC5_window_config_allows_overlay_title_bar_and_dragging(self):
        conf = json.loads(read(POD_BOARD / "src-tauri/tauri.conf.json"))
        main = next(w for w in conf["app"]["windows"] if w["label"] == "main")
        self.assertEqual(main.get("titleBarStyle"), "Overlay")
        self.assertTrue(main.get("hiddenTitle"))
        cap = json.loads(read(POD_BOARD / "src-tauri/capabilities/default.json"))
        self.assertIn("core:window:allow-start-dragging", cap["permissions"])

    # AC7 (sprite part), Decision 3: "one inline Lucide sprite, stroke 1.75" ; Decision 2 / Gaps: Phosphor duotone goes away.
    def test_AC7_icons_are_lucide_stroke_175_not_phosphor_duotone(self):
        html = read(UI / "index.html")
        self.assertNotIn('viewBox="0 0 256 256"', html, "Phosphor sprite (256 grid) still present in index.html")
        self.assertNotIn('viewBox="0 0 256 256"', read(UI / "popover.html"))
        self.assertRegex(html, r"stroke-width[=:]\s*[\"']?1\.75")

    # AC7: "No lane has a state-coloured background."  Decision 7: "Lanes: neutral, on `--bg`, radius 24".
    def test_AC7_lanes_are_neutral_on_bg_with_radius_24(self):
        html = strip_comments(read(UI / "index.html"))
        rules = re.findall(r"([^{}]+)\{([^{}]*)\}", html)
        lane = [(s.strip(), b) for s, b in rules if re.search(r"(^|[\s,>+~])\.lane(?![\w-])", s)]
        self.assertTrue(lane, "no .lane rule found")
        for s, b in lane:
            m = re.search(r"background(?:-color)?\s*:\s*([^;]+)", b)
            if m:
                self.assertNotRegex(m.group(1), r"--(session|rose|sky|sage|peach|accent|danger|warning|status)", f"{s}: state-coloured lane background")
        joined = " ".join(b for _, b in lane)
        self.assertIn("var(--bg)", joined)
        self.assertRegex(joined, r"border-radius\s*:\s*(24px|var\(--radius-card\))")

    # Decision 8: "The green left stripe goes away".  Decision 14 "the peach summary": the summary row stays neutral.
    def test_AC8_no_left_stripe_and_no_peach_summary(self):
        css = strip_comments(read(UI / "index.html"))
        self.assertNotRegex(css, r"border-left\s*:\s*[2-9]px[^;]*(success|green)|box-shadow\s*:\s*inset\s*[3-9]px")
        self.assertNotIn("--peach", css)

    # AC16 docs: "`README.md`, `GUIDE.md` and `THIRD-PARTY.md` describe the new look, fonts and icons."
    def test_AC16_third_party_lists_outfit_jetbrains_lucide_and_drops_old(self):
        t = read(POD_BOARD / "THIRD-PARTY.md")
        for word in ("Outfit", "JetBrains Mono", "Lucide", "ISC"):
            self.assertIn(word, t)
        for word in ("Ubuntu", "DejaVu", "Phosphor"):
            self.assertNotIn(word, t)

    def test_AC16_readme_and_guide_describe_new_look_not_old(self):
        for name in ("README.md", "GUIDE.md"):
            t = read(POD_BOARD / name)
            for word in ("Phosphor", "Ubuntu", "DejaVu", "grain"):
                self.assertNotIn(word, t, f"{name} still mentions {word}")
        self.assertIn("Lucide", read(POD_BOARD / "README.md"))
        self.assertRegex(read(POD_BOARD / "GUIDE.md"), r"title bar|Outfit|Lucide|Pod Cat")

    # Decision 5 (owner, confirmed): "Pod Board design notes go in a new `apps/pod-board/DESIGN.md`, not Shape Board's `DESIGN_SYSTEM.md`".
    def test_AC16_design_notes_live_in_pod_board_design_md(self):
        self.assertTrue((POD_BOARD / "DESIGN.md").is_file())


@needs_chrome
class TestBoardPage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.p = probe_page("index.html", "board")

    # AC4: "the computed `font-family` of `body` starts with `Outfit`."
    def test_AC4_body_font_is_outfit_and_files_load(self):
        self.assertEqual(self.p["errors"], [], "probe script error")
        self.assertTrue(re.match(r"\"?Outfit\"?\s*,", self.p["bodyFont"]), self.p["bodyFont"])
        self.assertTrue(self.p["fonts"]["outfit400"] and self.p["fonts"]["outfit500"] and self.p["fonts"]["outfit600"], self.p["fonts"])
        self.assertTrue(self.p["fonts"]["jbmono400"], self.p["fonts"])

    # AC3 / Decision 14 in rendered form: no uppercase label, no blur, no weight above 600 on any rendered text.
    def test_AC3_no_uppercase_blur_or_heavy_weight_rendered(self):
        self.assertEqual(self.p["bad"], [])

    # AC5: "The window shows, from left to right: title bar with Pod Cat logo and "Pod Board › Overview", a 256 px
    # sage sidebar, a main panel."  Decision 4: 48 px title bar; sidebar 256 on `--side`, radius 28; main radius 28; gap 12.
    def test_AC5_title_bar_48px_with_logo_and_breadcrumb(self):
        bars = [b for b in self.p["titlebars"] if b["rect"] and abs(b["rect"]["h"] - 48) < 0.5]
        self.assertTrue(bars, f"no [data-tauri-drag-region] element 48 px high: {self.p['titlebars']}")
        ok = [b for b in bars if re.search(r"Pod Board\s*›\s*Overview", b["text"]) and b["hasGraphic"]]
        self.assertTrue(ok, f"title bar needs the logo and 'Pod Board › Overview': {bars}")
        self.assertLess(ok[0]["rect"]["y"], 1)

    def test_AC5_sidebar_256_main_panel_and_shell_geometry(self):
        L = self.p["layout"]
        self.assertAlmostEqual(L["side"]["w"], 256, delta=0.5)
        self.assertEqual(L["sideRadius"], "28px")
        self.assertEqual(L["mainRadius"], "28px")
        self.assertEqual(L["shellGap"], "12px")
        self.assertGreaterEqual(L["main"]["x"], L["side"]["x"] + L["side"]["w"])
        self.assertGreaterEqual(L["side"]["y"], 47)

    # Decision 4: "The brand block in the sidebar goes away."
    def test_AC5_brand_block_gone(self):
        self.assertFalse(self.p["brandText"])

    # AC6: "Light theme: page `#F7F0E6`, main panel `#F2E9DC`, cards `#FFFBF5`, sidebar `#E6E8D6`. Dark theme: `#211A15`,
    # `#2A211B`, `#32281F`, `#2C2A1F`."  (cards = `--surface-2`, checked on its token; no cards without data)
    def test_AC6_light_surfaces(self):
        s = self.p["light"]
        self.assertEqual(s["body"], hex_to_css_rgb("#F7F0E6"))
        self.assertEqual(s["main"], hex_to_css_rgb("#F2E9DC"))
        self.assertEqual(s["side"], hex_to_css_rgb("#E6E8D6"))

    def test_AC6_dark_surfaces(self):
        s = self.p["dark"]
        self.assertEqual(s["body"], hex_to_css_rgb("#211A15"))
        self.assertEqual(s["main"], hex_to_css_rgb("#2A211B"))
        self.assertEqual(s["side"], hex_to_css_rgb("#2C2A1F"))

    @unittest.skipUnless(CHROME, "")
    def test_AC6_card_surface_tokens(self):
        v = token_values(UI / "theme.css", ["--surface-2"])
        self.assertEqual(norm(v["light"]["--surface-2"]), "#fffbf5")
        self.assertEqual(norm(v["dark"]["--surface-2"]), "#32281f")

    # AC6: "The switch in the sidebar foot changes it" (light, dark, system), Decision 5.
    def test_AC6_theme_switch_is_in_sidebar_and_changes_theme(self):
        self.assertEqual(self.p["themeButtonsInSide"], [True, True, True])
        self.assertEqual(self.p["afterDarkClick"], "dark")
        self.assertEqual(self.p["afterLightClick"], "light")

    # AC10: "The cat is in the summary row tile ... Nothing sits in the bottom-right corner."
    def test_AC10_cat_is_not_a_fixed_corner_tile(self):
        self.assertNotIn("fixed", self.p["cats"])

    # AC11: "Sidebar collapse (button and ⌘B) still works and is remembered. Collapsed, each item has a tooltip with its name."
    def test_AC11_sidebar_collapses_expands_and_items_have_tooltips(self):
        L, c, r = self.p["layout"], self.p["collapsed"], self.p["reexpanded"]
        self.assertAlmostEqual(L["side"]["w"], 256, delta=0.5)
        self.assertEqual(c["expanded"], "false")
        self.assertLess(c["sideW"], 256)
        self.assertTrue(c["tips"])
        for t in c["tips"]:
            self.assertTrue(t["title"], f"collapsed item without a tooltip: {t}")
        self.assertEqual(r["expanded"], "true")
        self.assertAlmostEqual(r["sideW"], 256, delta=0.5)

    # AC14: "Tab reaches every control in the window ..., and each one shows a 2 px ring."  Decision 13: "2 px `--ring`
    # outline with 2 px offset on every button, card, row and link."
    def _ring(self, theme):
        s = self.p[theme]
        self.assertTrue(self.p["focusVisibleWorks"], "headless Chrome did not match :focus-visible; test cannot judge")
        self.assertTrue(s["focus"], "no controls found")
        bad = [f for f in s["focus"] if not (f["style"] == "solid" and f["width"] == "2px" and f["offset"] == "2px" and f["color"] == s["ringRgb"])]
        self.assertEqual(bad, [], f"controls without the 2 px ring ({theme}, ring={s['ringRgb']})")

    def test_AC14_focus_ring_light(self):
        self._ring("light")

    def test_AC14_focus_ring_dark(self):
        self._ring("dark")


@needs_chrome
class TestPopoverPage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.p = probe_page("popover.html", "popover")

    # AC4: "the window and the popover show Outfit text"
    def test_AC4_popover_font_is_outfit(self):
        self.assertTrue(re.match(r"\"?Outfit\"?\s*,", self.p["bodyFont"]), self.p["bodyFont"])
        self.assertTrue(self.p["fonts"]["outfit400"], self.p["fonts"])

    def test_AC3_popover_has_no_uppercase_blur_or_heavy_weight(self):
        self.assertEqual(self.p["bad"], [])

    # AC6: "... the popover follows." The popover uses the same page colour per theme.
    def test_AC6_popover_follows_theme_tokens(self):
        self.assertEqual(self.p["light"]["body"], hex_to_css_rgb("#F7F0E6"))
        self.assertEqual(self.p["dark"]["body"], hex_to_css_rgb("#211A15"))

    # AC14: "each one shows a 2 px ring" in the popover too.
    def test_AC14_popover_focus_ring(self):
        self.assertTrue(self.p["focusVisibleWorks"])
        for theme in ("light", "dark"):
            s = self.p[theme]
            bad = [f for f in s["focus"] if not (f["style"] == "solid" and f["width"] == "2px" and f["offset"] == "2px" and f["color"] == s["ringRgb"])]
            self.assertEqual(bad, [], theme)


if __name__ == "__main__":
    if not CHROME:
        print("NOTE: no Chrome/Chromium found; headless page checks are skipped", file=sys.stderr)
    unittest.main(verbosity=2)
