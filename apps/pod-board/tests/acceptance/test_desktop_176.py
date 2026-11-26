"""Acceptance tests for GitHub issue #176: Pod Board shows Claude desktop app sessions (Desktop source, follow-up of #137).

Run: bash apps/pod-board/tests/acceptance/run_176.sh
That runner runs the #137 file (test_sources_and_steps.py) with the Desktop rows required (POD_BOARD_SKIP_DESKTOP cleared,
Chrome found or the run fails), then this file. This file holds the static checks of README.md and GUIDE.md.

The `cargo test` criterion is checked against the fixtures in src-tauri/tests/fixtures/desktop (README there); the Mac-only
criteria are manual checks on M0 (README.md in this folder).

Test names: AC<n> counts the boxes of the ticket's "Acceptance" list from the top (1 = the `cargo test` box ... 6 = the
"exit 0" box).
"""
import re
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
POD_BOARD = HERE.parent.parent


def read(p):
    return Path(p).read_text(encoding="utf-8")


def flat(s):
    return re.sub(r"\s+", " ", s)


class Docs(unittest.TestCase):
    """AC5: "README and GUIDE describe the Desktop source, what is read, and what was not verified." """

    def section(self, text, name):
        m = re.search(rf"^## {re.escape(name)}\n(.*?)(?=^## |\Z)", text, flags=re.S | re.M)
        self.assertIsNotNone(m, f"no '## {name}' section")
        return flat(m.group(1))

    # AC5 "what is read" + What to build 7: 'README "Data and privacy" lists the new file.'
    def test_AC5_readme_data_and_privacy_lists_the_desktop_session_files(self):
        s = self.section(read(POD_BOARD / "README.md"), "Data and privacy")
        self.assertIn("claude-code-sessions", s, "the desktop app's session folder is not listed")
        self.assertIn("local_", s, "the desktop app's local_<uuid>.json files are not listed")

    # AC5 "describe the Desktop source"
    def test_AC5_readme_describes_the_desktop_source_as_shown(self):
        readme = read(POD_BOARD / "README.md")
        s = self.section(readme, "What it does")
        self.assertIn("`Desktop`", s, "the What it does section does not name the `Desktop` chip")
        self.assertNotRegex(s, r"(?i)desktop source[^.]*is not shown", "README still says the Desktop source is not shown")
        self.assertNotIn("No Desktop source", flat(readme), "README Limits still says there is no Desktop source")

    # AC5 "what was not verified"
    def test_AC5_readme_limits_say_what_was_not_verified_for_desktop_sessions(self):
        s = self.section(read(POD_BOARD / "README.md"), "Limits")
        bullets = [b for b in re.split(r"(?:^| )- ", s) if re.search(r"(?i)not verif|does not verify", b)]
        self.assertTrue(any(re.search(r"(?i)desktop(-app| app)? sessions?", b) for b in bullets),
                        "no 'not verified' line in Limits names the desktop sessions")
        self.assertNotIn("Desktop is left out", s, "Limits still says the Desktop source is left out")

    # AC5 (GUIDE): "describe the Desktop source"
    def test_AC5_guide_describes_the_desktop_source(self):
        g = flat(read(POD_BOARD / "GUIDE.md"))
        self.assertIn("Desktop", g)
        self.assertNotRegex(g, r"(?i)desktop-app sessions are not shown", "GUIDE still says desktop sessions are not shown")
        self.assertRegex(g, r"\*\*Source\*\* \([^)]*Desktop", "the GUIDE's Source filter does not list Desktop")


if __name__ == "__main__":
    unittest.main(verbosity=2)
