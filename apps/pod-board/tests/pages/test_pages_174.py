"""Page test for issue #174: the Stopped strip's chip for a machine the app cannot reach.

Run: python3 -I apps/pod-board/tests/pages/test_pages_174.py

A headless Chrome loads a copy of the window page with a mock Tauri runtime and dumps the machine chips. A machine that
Pod Bay lists keeps its own words (`offline`, `password out of date`); when the app has no route to it, one more chip
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
    { name: 'M0', online: true, key_stale: false, unreachable: false },
    { name: 'U0', online: false, key_stale: false, unreachable: true },
    { name: 'UA', online: false, key_stale: true, unreachable: true },
    { name: 'UB', online: false, key_stale: false, unreachable: false },
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
    d = Path(tempfile.mkdtemp(prefix="pod-board-174-"))
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
class UnreachableChip(unittest.TestCase):
    def test_a_machine_with_no_route_keeps_its_own_words_and_gets_a_neutral_line(self):
        texts = [c["text"] for c in chips()]
        # Pod Bay's own words stay for every machine it lists as offline.
        self.assertIn("U0 offline", texts)
        self.assertIn("UA offline · password out of date", texts)
        self.assertIn("UB offline", texts)
        # No route from this app: a neutral line, no claim about the cause, no instruction to change a setting.
        self.assertIn("U0 unreachable from Pod Board (no route to host)", texts)
        self.assertIn("UA unreachable from Pod Board (no route to host)", texts)
        self.assertNotIn("UB unreachable from Pod Board (no route to host)", texts)
        self.assertFalse([t for t in texts if "allow Local Network" in t], texts)
        self.assertFalse([t for t in texts if t.startswith("M0")], "an online machine has no chip")


if __name__ == "__main__":
    unittest.main()
