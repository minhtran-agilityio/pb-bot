"""Page test for issue #177: the Stopped strip's chip for a machine Pod Bay gave no address for.

Run: python3 -I apps/pod-board/tests/pages/test_pages_177.py

A headless Chrome loads a copy of the window page with a mock Tauri runtime and dumps the machine chips. A machine that Pod Bay
lists offline keeps its own words; when Pod Bay gave no address and no rows for it, the app cannot probe it, and one more chip
says so without naming a cause. Skipped, loudly, when no Chrome/Chromium binary exists.
"""
import json
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

UI = Path(__file__).resolve().parent.parent.parent / "ui"
MAC_CHROME = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
CHROME = next((p for p in (shutil.which(n) for n in ("google-chrome", "chromium", "chromium-browser", "google-chrome-stable")) if p),
              str(MAC_CHROME) if MAC_CHROME.is_file() else None)

MOCK = r"""
(() => {
  const machines = [
    { name: 'M0', online: true, key_stale: false, unreachable: false, no_address: false },
    { name: 'U0', online: false, key_stale: true, unreachable: false, no_address: true },
    { name: 'UB', online: false, key_stale: false, unreachable: false, no_address: false },
    { name: 'UD', online: false, key_stale: false, unreachable: true, no_address: false },
  ];
  const snap = { ok: true, error: null, read_at: Date.now(), dnd_until: null, counts: { needs: 0, busy: 0, idle: 0, stopped: 0, finished: 0 },
                 sessions: [], stopped: [], machines };
  window.__TAURI__ = {
    core: { invoke: (cmd) => Promise.resolve(cmd === 'snapshot' ? JSON.parse(JSON.stringify(snap)) : null) },
    event: { listen: () => Promise.resolve(() => {}) },
  };
})();
"""

PROBE = r"""
(async () => {
  await new Promise((r) => setTimeout(r, 800));
  const pre = document.createElement('pre'); pre.id = '__probe';
  pre.textContent = JSON.stringify([...document.querySelectorAll('.strip .chip.off')].map((c) => ({ text: c.textContent, title: c.title })));
  document.body.append(pre);
})();
"""


def chips():
    d = Path(tempfile.mkdtemp(prefix="pod-board-177-"))
    try:
        ui = d / "ui"
        shutil.copytree(UI, ui)
        src = (ui / "index.html").read_text(encoding="utf-8")
        src = src.replace("<head>", f"<head><script>{MOCK}</script>", 1).replace("</body>", f"<script>{PROBE}</script></body>")
        (ui / "index.html").write_text(src, encoding="utf-8")
        cmd = [CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--allow-file-access-from-files", "--window-size=1180,720",
               "--virtual-time-budget=30000", f"--user-data-dir={d / 'profile'}", "--dump-dom", (ui / "index.html").as_uri()]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    finally:
        shutil.rmtree(d, ignore_errors=True)
    m = re.search(r'<pre id="__probe">(.*?)</pre>', r.stdout, flags=re.S)
    if not m:
        raise AssertionError(f"probe produced no result (chrome exit {r.returncode}): {r.stderr[-300:]}")
    import html
    return json.loads(html.unescape(m.group(1)))


@unittest.skipUnless(CHROME, "no Chrome/Chromium binary: the page test did not run")
class NoAddressChip(unittest.TestCase):
    def test_a_machine_with_no_address_says_it_cannot_be_reached_and_names_no_cause(self):
        got = {c["text"]: c["title"] for c in chips()}
        # Pod Bay's own words stay.
        self.assertIn("U0 offline · password out of date", got)
        self.assertIn("UB offline", got)
        # No rows and no address: the app says it could not reach the machine, and says what it cannot tell apart.
        line = "U0 unreachable from Pod Board (Pod Bay gave no address)"
        self.assertIn(line, got)
        title = got[line]
        for cause in ("off", "password", "Local Network"):
            self.assertIn(cause, title)
        self.assertIn("cannot tell", title)
        # Only the machine marked `no_address` gets it; the #174 line stays for a machine that had an address.
        self.assertNotIn("UB unreachable from Pod Board (Pod Bay gave no address)", got)
        self.assertIn("UD unreachable from Pod Board (no route to host)", got)
        self.assertNotIn("UD unreachable from Pod Board (Pod Bay gave no address)", got)
        self.assertFalse([t for t in got if t.startswith("M0")], "an online machine has no chip")


if __name__ == "__main__":
    unittest.main()
