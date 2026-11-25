"""Acceptance tests for GitHub issue #137: Pod Board shows where a session runs, why it waits, what it does now
and what its sub-agents do.

Run: python3 -I apps/pod-board/tests/acceptance/test_sources_and_steps.py   (or: bash apps/pod-board/tests/acceptance/run_137.sh)

Outside-in and headless: a headless Chrome loads copies of the two pages with a mock Tauri runtime (mock137.js; the
field names are the "Mock contract" in README.md), and static checks read README.md and GUIDE.md. Nothing in the app
is changed. Chrome tests are skipped, loudly, when no Chrome/Chromium binary exists.

Desktop tests are grouped in the classes whose names end in "Desktop". Skip them with POD_BOARD_SKIP_DESKTOP=1
(owner decision: when no Claude desktop-app sessions are found, the Desktop source is left out and its rows dropped).
The Rust criteria (`cargo test`) and the Mac-only criteria are not tested here: see README.md.
"""
import html
import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
POD_BOARD = HERE.parent.parent
UI = POD_BOARD / "ui"
CHROME = next((p for p in (shutil.which(n) for n in ("google-chrome", "chromium", "chromium-browser", "google-chrome-stable")) if p), None)
SKIP_DESKTOP = os.environ.get("POD_BOARD_SKIP_DESKTOP") == "1"

needs_chrome = unittest.skipUnless(CHROME, "no google-chrome/chromium on this machine: headless page checks skipped")
needs_desktop = unittest.skipIf(SKIP_DESKTOP, "POD_BOARD_SKIP_DESKTOP=1: the Desktop source is left out (owner decision)")

_tmp = []


def tearDownModule():
    for d in _tmp:
        shutil.rmtree(d, ignore_errors=True)


def read(p):
    return Path(p).read_text(encoding="utf-8")


_cache = {}


def probe(page, mode, desktop):
    key = (page, mode, desktop)
    if key in _cache:
        return _cache[key]
    d = Path(tempfile.mkdtemp(prefix="pod-board-137-"))
    _tmp.append(str(d))
    ui = d / "ui"
    shutil.copytree(UI, ui)
    src = read(ui / page)
    head = f"<script>window.__WITH_DESKTOP={json.dumps(desktop)};</script><script>{read(HERE / 'mock137.js')}</script>"
    tail = f"<script>window.__PROBE_MODE={json.dumps(mode)};</script><script>{read(HERE / 'probe137.js')}</script>"
    src = src.replace("<head>", "<head>" + head, 1).replace("</body>", tail + "</body>")
    (ui / page).write_text(src, encoding="utf-8")
    prof = d / "profile"
    cmd = [CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--allow-file-access-from-files",
           "--window-size=1180,900", "--virtual-time-budget=20000", f"--user-data-dir={prof}", "--dump-dom", (ui / page).as_uri()]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    m = re.search(r'<pre id="__probe">(.*?)</pre>', r.stdout, flags=re.S)
    if not m:
        raise AssertionError(f"probe produced no result (chrome exit {r.returncode}): {r.stderr[-300:]}")
    out = json.loads(html.unescape(m.group(1)))
    _cache[key] = out
    return out


def board(desktop=False):
    out = probe("index.html", "board", desktop)
    if out["errors"]:
        raise AssertionError(f"page script failed with the mock data: {out['errors']}")
    return out


def popover(desktop=False):
    out = probe("popover.html", "popover", desktop)
    if out["errors"]:
        raise AssertionError(f"popover script failed with the mock data: {out['errors']}")
    return out


def by_title(rows, title):
    for r in rows:
        if r["title"] == title:
            return r
    raise AssertionError(f"no card/row titled {title!r}; got {[r['title'] for r in rows]}")


def nav_item(cap, group, label):
    for i in cap["nav"]:
        if i["group"] == group and i["label"].startswith(label):
            return i
    raise AssertionError(f"no sidebar row {label!r} under group {group!r}; got {[(i['group'], i['label']) for i in cap['nav']]}")


def state_counts(cap):
    return {l: nav_item({"nav": [i for i in cap["nav"] if i["group"] == ""]}, "", l)["count"] for l in ("Needs you", "Working", "Idle")}


CHIP = {"Fix login": "Pod Bay · M0", "Build local reader": "Pod Bay · M0", "Review notes": "Pod Bay · U0", "Remote worker": "Pod Bay · U0",
        "Pick a database": "Terminal · RC", "Terminal tidy": "Terminal · RC", "Notes": "Terminal · RC"}
BASE_TITLES = sorted(CHIP)


@needs_chrome
class SourceChips(unittest.TestCase):
    # AC7: "On the app: every card, list row, peek head and popover row shows its source. A Pod Bay chip shows the machine."
    def test_AC7_every_card_shows_its_source(self):
        cap = board()["home"]
        self.assertEqual(sorted(cap["titles"]), BASE_TITLES)
        for title, chip in CHIP.items():
            self.assertIn(chip, by_title(cap["rows"], title)["text"], title)

    # AC7 + Changes 2/Gaps: "Source chip, then the project. The machine shows only on a Pod Bay chip"
    def test_AC7_machine_shows_only_on_a_pod_bay_chip(self):
        cap = board()["home"]
        for title in ("Pick a database", "Terminal tidy", "Notes"):
            self.assertNotIn("M0", by_title(cap["rows"], title)["text"], f"{title}: machine must not show on a Terminal chip")
        self.assertIn("pb-bot", by_title(cap["rows"], "Fix login")["text"])

    # AC7: "every ... list row ... shows its source" and Changes 2: "list rows (icon only, with a tooltip)"
    def test_AC7_list_rows_show_an_icon_with_a_tooltip(self):
        cap = board()["list"]
        self.assertEqual(sorted(r["title"] for r in cap["lrows"]), BASE_TITLES)
        for title, chip in CHIP.items():
            r = by_title(cap["lrows"], title)
            self.assertTrue(any(chip in (t["title"] + " " + t["aria"]) and t["svg"] for t in r["tips"]), f"{title}: no icon with tooltip {chip!r}: {r['tips']}")
            self.assertNotIn(chip, r["text"], f"{title}: list row shows the source as icon only")

    # AC7: "every ... peek head ... shows its source"
    def test_AC7_peek_head_shows_the_source(self):
        pk = board()["peek"]
        self.assertIn("Pod Bay · M0", pk["Fix login"]["head"])
        self.assertIn("Terminal · RC", pk["Pick a database"]["head"])

    # AC7: "every ... popover row shows its source. A Pod Bay chip shows the machine."
    def test_AC7_popover_rows_show_the_source(self):
        rows = popover()["rows"]
        for title, chip in (("Fix login", "Pod Bay · M0"), ("Review notes", "Pod Bay · U0"), ("Pick a database", "Terminal · RC")):
            row = next((r for r in rows if title in r["text"]), None)
            self.assertIsNotNone(row, f"no popover row for {title}: {rows}")
            self.assertIn(chip, row["text"])


@needs_chrome
class SidebarFilters(unittest.TestCase):
    # AC8: "a click on "Desktop" under Source shows only desktop sessions; a second click clears it." (here with Terminal;
    # the Desktop case is in FiltersDesktop). Changes 3: groups "Source" and "Projects" replace "Machines".
    def test_AC8_source_filter_toggles_and_counts_match(self):
        f = board()["filters"]["Source/Terminal"]
        self.assertTrue(f["clicked"], "no 'Terminal' row under the Source group")
        self.assertEqual(sorted(f["on"]["titles"]), ["Notes", "Pick a database", "Terminal tidy"])
        self.assertEqual(sorted(f["off"]["titles"]), BASE_TITLES)
        self.assertEqual(state_counts(f["on"]), {"Needs you": "1", "Working": "0", "Idle": "2"})
        self.assertEqual(state_counts(f["off"]), {"Needs you": "3", "Working": "2", "Idle": "2"})

    # AC8: "The breadcrumb names the filter."
    def test_AC8_breadcrumb_names_the_filter(self):
        f = board()["filters"]["Source/Terminal"]
        self.assertIn("Terminal", f["on"]["crumb"])
        self.assertNotIn("Terminal", f["off"]["crumb"])
        g = board()["filters"]["Projects/alpha"]
        self.assertIn("alpha", g["on"]["crumb"])
        self.assertNotIn("alpha", g["off"]["crumb"])

    # AC8: "The same for a project." and "Counts in the sidebar match the view."
    def test_AC8_project_filter_toggles_and_counts_match(self):
        f = board()["filters"]["Projects/alpha"]
        self.assertTrue(f["clicked"], "no 'alpha' row under the Projects group")
        self.assertEqual(f["on"]["titles"], ["Pick a database"])
        self.assertEqual(sorted(f["off"]["titles"]), BASE_TITLES)
        self.assertEqual(state_counts(f["on"]), {"Needs you": "1", "Working": "0", "Idle": "0"})

    # AC8: sidebar rows hold counts; Changes 3: "Each row is a filter that toggles; "Clear filter" stays."
    def test_AC8_filter_rows_have_counts_and_clear_filter_stays(self):
        cap = board()
        home = cap["home"]
        self.assertEqual(nav_item(home, "Source", "Pod Bay")["count"], "4")
        self.assertEqual(nav_item(home, "Source", "Terminal")["count"], "3")
        self.assertEqual(nav_item(home, "Projects", "beta")["count"], "2")
        on = cap["filters"]["Source/Pod Bay"]["on"]
        self.assertTrue(any(i["label"] == "Clear filter" for i in on["nav"]), "no 'Clear filter' row while a filter is on")
        self.assertEqual(sorted(on["titles"]), ["Build local reader", "Fix login", "Remote worker", "Review notes"])

    # Changes 3: 'groups "Source" and "Projects" replace "Machines"'; Changes 7 / Gaps: "Offline machines keep their chips in the
    # Stopped strip"; Gaps: "Keep both" the stopped count and the Stopped page.
    def test_AC16_machines_group_gone_and_offline_chip_stays_in_stopped_strip(self):
        home = board()["home"]
        self.assertNotIn("Machines", {i["group"] for i in home["nav"]})
        self.assertIn("Source", {i["group"] for i in home["nav"]})
        self.assertIn("Projects", {i["group"] for i in home["nav"]})
        # AC16: "On the app: with one machine offline, its chip still shows in the Stopped strip."
        self.assertIn("U1 offline · password out of date", home["strip"])
        self.assertIn("pb-bot/old", home["strip"])
        self.assertTrue(any(c.endswith("stopped") for c in home["counts"]), home["counts"])
        self.assertTrue(any(i["label"].startswith("Stopped") for i in home["nav"]), "the Stopped page is gone from the sidebar")


@needs_chrome
class ProjectDot(unittest.TestCase):
    # AC9: "On the app: a project with a waiting session has an accent dot, and its tooltip says so."
    def test_AC9_project_with_a_waiting_session_has_an_accent_dot_and_a_tooltip(self):
        cap = board()
        home = cap["home"]
        for project in ("pb-bot", "alpha", "gamma"):
            i = nav_item(home, "Projects", project)
            self.assertEqual(i["dotBg"], cap["accent"], f"{project}: dot is not the accent colour")
            self.assertRegex(i["title"] + " " + i["aria"] + " " + i["dotText"], r"(?i)need|wait", f"{project}: no tooltip or label that says a session waits")
        beta = nav_item(home, "Projects", "beta")
        self.assertNotEqual(beta["dotBg"], cap["accent"])
        self.assertNotRegex(beta["title"] + " " + beta["dotText"], r"(?i)need|wait")


@needs_chrome
class WaitReason(unittest.TestCase):
    # AC11: "On the app: a session that asks for a permission shows the reason on its card, in the peek and in the popover row."
    def test_AC11_reason_on_the_card(self):
        cap = board()["home"]
        a = by_title(cap["rows"], "Fix login")
        self.assertIn("Permission", a["text"])
        self.assertIn("git push origin main", a["text"])
        self.assertIn("input needed", a["text"], "the input needed tag stays")
        self.assertIn("Which database?", by_title(cap["rows"], "Pick a database")["text"])
        self.assertIn("Question", by_title(cap["rows"], "Pick a database")["text"])

    # Changes 4: "kind in accent text, then the detail (a command in the code font)"
    def test_AC11_kind_in_accent_text_and_command_in_code_font(self):
        cap = board()
        a = by_title(cap["home"]["rows"], "Fix login")
        self.assertTrue(a["kinds"], "no kind label")
        self.assertEqual(a["kinds"][0]["color"], cap["accentText"])
        self.assertTrue(a["mono"], "detail not found as its own element")
        self.assertIn("JetBrains Mono", a["mono"][0]["ff"])

    # Changes 4: "When no reason is known, the line is left out and the `input needed` tag stays."
    def test_AC11_no_reason_means_no_line_and_the_tag_stays(self):
        rows = board()["home"]["rows"]
        self.assertIn("Permission", by_title(rows, "Fix login")["text"], "a known reason must show (so the missing line below means something)")
        e = by_title(rows, "Review notes")
        self.assertIn("input needed", e["text"])
        self.assertNotRegex(e["text"], r"Permission|Question|Plan approval")

    # AC11 (peek) and Changes 4: 'The peek shows it in a "Why it waits" box.'
    def test_AC11_reason_in_the_peek(self):
        pk = board()["peek"]["Fix login"]
        self.assertIn("Why it waits", pk["body"])
        self.assertIn("Permission", pk["body"])
        self.assertIn("git push origin main", pk["body"])
        self.assertNotIn("Why it waits", board()["peek"]["Build local reader"]["body"])

    # AC11 (popover row)
    def test_AC11_reason_in_the_popover_row(self):
        row = next(r for r in popover()["rows"] if "Fix login" in r["text"])
        self.assertIn("Permission", row["text"])
        self.assertIn("git push origin main", row["text"])

    # Gaps: "show the badge only where no lane head does (the popover and a filtered list)": none in a lane card.
    def test_AC11_no_needs_you_badge_in_a_lane_card(self):
        a = by_title(board()["home"]["rows"], "Fix login")
        self.assertIn("git push origin main", a["text"], "the card must show its reason (so the missing badge below means something)")
        self.assertNotRegex(a["text"].lower(), r"needs you")

    # Gaps: "show the badge only where no lane head does (the popover ...)"
    def test_AC11_needs_you_badge_in_the_popover_row(self):
        row = next(r for r in popover()["rows"] if "Fix login" in r["text"])
        self.assertRegex(row["text"].lower(), r"needs you")


@needs_chrome
class StepAndSubagents(unittest.TestCase):
    # AC10: "Its card shows the step line, 3 rows and "+1 more running"" (4 running sub-agents)
    def test_AC10_card_shows_step_line_three_rows_and_more_running(self):
        b = by_title(board()["home"]["rows"], "Build local reader")["text"]
        self.assertIn("Edit: src/local.rs", b)
        for t in ("scan model", "write tests", "draft plan"):
            self.assertIn(t, b)
        self.assertNotIn("check docs", b, "a 4th running row must not show")
        self.assertNotIn("nested lookup", b, "a finished sub-agent is not a running row")
        self.assertIn("+1 more running", b)
        self.assertIn("4 sub-agents running", b)

    # Changes 5: "Step line on a working card, when the current step is known. Otherwise the Pod Bay description shows, as today."
    def test_AC10_step_replaces_description_only_when_known(self):
        rows = board()["home"]["rows"]
        self.assertNotIn("POD BAY DESC B", by_title(rows, "Build local reader")["text"])
        self.assertIn("POD BAY DESC G", by_title(rows, "Remote worker")["text"])

    # Changes 6: "tags `n sub-agents running` or `n sub-agents done`"
    def test_AC10_done_tag(self):
        self.assertIn("2 sub-agents done", by_title(board()["home"]["rows"], "Remote worker")["text"])

    # AC10: "the summary count rises by 4" and Changes 6/Gaps: 'Summary counts: working, sub-agents, idle' (stopped kept)
    def test_AC10_summary_counts_working_subagents_idle_stopped(self):
        c = board()["home"]["counts"]
        self.assertEqual(len(c), 4, c)
        self.assertEqual([x.split(" ", 1)[1] for x in c], ["working", "sub-agents", "idle", "stopped"])
        self.assertEqual([x.split(" ", 1)[0] for x in c], ["2", "4", "2", "1"])

    # AC10: "the peek lists all 4 with model and time, and marks each done when it ends."
    def test_AC10_peek_lists_every_subagent_with_model_and_done_marks(self):
        t = board()["peek"]["Build local reader"]["tasks"]
        for task, model in (("scan model", "haiku"), ("write tests", "sonnet"), ("draft plan", "opus"), ("check docs", "haiku"), ("nested lookup", "haiku")):
            self.assertIsNotNone(t[task], f"{task} missing in the peek")
            self.assertIn(model, t[task]["rowText"])
            self.assertRegex(t[task]["rowText"], r"\d+\s*(s|m)\b", f"{task}: no time")
        self.assertRegex(t["nested lookup"]["rowText"], r"(?i)took")
        self.assertNotRegex(t["scan model"]["rowText"], r"(?i)took")

    # Changes 6: "the full tree in the peek" with depth (Gaps: "type, task, model, running or took, step or result")
    def test_AC10_peek_tree_shows_type_result_and_nesting(self):
        t = board()["peek"]["Build local reader"]["tasks"]
        self.assertTrue(t["scan model"] and t["nested lookup"], "sub-agents missing in the peek")
        self.assertIn("Explore", t["scan model"]["rowText"])
        self.assertIn("3 files", t["nested lookup"]["rowText"])
        self.assertGreater(t["nested lookup"]["left"], t["scan model"]["left"], "a nested sub-agent must be indented")

    # Changes 6: "`n sub-agents` in list rows and popover rows"
    def test_AC10_list_and_popover_rows_say_n_subagents(self):
        self.assertIn("4 sub-agents", by_title(board()["list"]["lrows"], "Build local reader")["text"])
        row = next(r for r in popover()["rows"] if "Build local reader" in r["text"])
        self.assertIn("4 sub-agents", row["text"])

    # Gaps: 'The project dot and the sub-agent dots show state by colour only. UDS: colour plus icon or word.' - the
    # peek rows above carry the words "took"/model; the card rows carry their task text (tested above).


@needs_chrome
class Layout(unittest.TestCase):
    # AC (Changes 8): "Lanes: 1.25 / 1.25 / 1."
    def test_AC_lanes_are_1_25_1_25_1(self):
        cols = [float(x[:-2]) for x in board()["home"]["cols"].split()]
        self.assertEqual(len(cols), 3, cols)
        self.assertAlmostEqual(cols[0] / cols[2], 1.25, delta=0.03)
        self.assertAlmostEqual(cols[1] / cols[2], 1.25, delta=0.03)


@needs_chrome
@needs_desktop
class FiltersDesktop(unittest.TestCase):
    """Desktop rows. Dropped when no desktop sessions are found (POD_BOARD_SKIP_DESKTOP=1)."""

    # AC7 (Desktop)
    def test_AC7_desktop_cards_show_the_source_without_machine(self):
        rows = board(True)["home"]["rows"]
        for title in ("Index repo", "Ask about schema"):
            r = by_title(rows, title)
            self.assertIn("Desktop", r["text"])
            self.assertNotIn("M0", r["text"])

    # AC8: "a click on "Desktop" under Source shows only desktop sessions; a second click clears it."
    def test_AC8_desktop_source_filter(self):
        f = board(True)["filters"]["Source/Desktop"]
        self.assertTrue(f["clicked"], "no 'Desktop' row under the Source group")
        self.assertEqual(sorted(f["on"]["titles"]), ["Ask about schema", "Index repo"])
        self.assertEqual(len(f["off"]["titles"]), 9)
        self.assertIn("Desktop", f["on"]["crumb"])
        self.assertEqual(state_counts(f["on"]), {"Needs you": "1", "Working": "1", "Idle": "0"})

    # AC9 (Desktop session makes its project accent): alpha holds a waiting terminal and a waiting desktop session
    def test_AC11_plan_reason_on_a_desktop_card(self):
        r = by_title(board(True)["home"]["rows"], "Ask about schema")["text"]
        self.assertIn("Plan approval", r)
        self.assertIn("Add a migration", r)

    # Changes 7: "Stopped strip: today's chips, plus desktop sessions closed in the last 12 hours."
    def test_AC_stopped_strip_lists_closed_desktop_sessions_and_keeps_chips(self):
        strip = board(True)["home"]["strip"]
        self.assertIn("Closed chat", strip)
        self.assertIn("pb-bot/old", strip)
        self.assertIn("U1 offline · password out of date", strip)


class Docs(unittest.TestCase):
    def section(self, text, name):
        m = re.search(rf"^## {re.escape(name)}\n(.*?)(?=^## |\Z)", text, flags=re.S | re.M)
        self.assertIsNotNone(m, f"no '## {name}' section")
        return m.group(1)

    # AC17: 'README.md ("What it does", "Data and privacy", "Limits") and GUIDE.md describe the new sources, what is read,
    # and what was not verified.' Changes 9: 'the README section "Data and privacy" lists every new file the app reads.'
    def test_AC17_readme_data_and_privacy_lists_the_new_files(self):
        s = self.section(read(POD_BOARD / "README.md"), "Data and privacy")
        self.assertRegex(s, r"~/\.claude/|\.claude/", "no file under ~/.claude is listed")
        self.assertRegex(s, r"(?i)sub-?agent", "sub-agent files are not listed")
        self.assertRegex(s, r"detail", "the `podbay-dev detail` read for working sessions is not described")
        self.assertRegex(s, r"(?i)\bhook", "does not say that no hook is added")

    def test_AC17_readme_what_it_does_describes_the_new_sources(self):
        s = self.section(read(POD_BOARD / "README.md"), "What it does")
        for w in ("Terminal", "Source", "sub-agent"):
            self.assertIn(w, s)

    @needs_desktop
    def test_AC17_readme_names_the_desktop_source(self):
        self.assertIn("Desktop", self.section(read(POD_BOARD / "README.md"), "What it does"))

    def test_AC17_readme_limits_say_what_was_not_verified(self):
        s = self.section(read(POD_BOARD / "README.md"), "Limits")
        self.assertRegex(s, r"(?i)sub-agent|waitingFor")

    def test_AC17_guide_describes_the_new_sources(self):
        g = read(POD_BOARD / "GUIDE.md")
        for w in ("Source", "Projects", "sub-agent"):
            self.assertIn(w, g)


if __name__ == "__main__":
    unittest.main(verbosity=2)
