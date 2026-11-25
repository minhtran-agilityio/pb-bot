"""Page tests for issue #158, bugs 5 and 6: one rule for the waiting tag on every surface, and one row key per session.

Run: python3 -I apps/pod-board/tests/pages/test_pages_158.py

A headless Chrome loads copies of the two pages with a mock Tauri runtime (below) and dumps what the probe found. Skipped,
loudly, when no Chrome/Chromium binary exists. Nothing in the app is changed.
"""
import html
import json
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

UI = Path(__file__).resolve().parent.parent.parent / "ui"
CHROME = next((p for p in (shutil.which(n) for n in ("google-chrome", "chromium", "chromium-browser", "google-chrome-stable")) if p), None)

# Two unnamed terminal sessions in one project: same machine, same title (the project), different Remote Control ids.
# A Pod Bay session waits with a known reason; another waits with none.
MOCK = r"""
(() => {
  const now = Date.now(), MIN = 60000;
  const s = (o) => Object.assign({ ws: '', machine: 'M0', waiting: '', what: '', branch: '', links: [], prs: 0, queued: 0, name: '' }, o);
  const sessions = [
    s({ title: 'alpha', project: 'alpha', source: 'terminal', rc_id: 'session_01A', state: 'idle', since: now - 30 * MIN }),
    s({ title: 'alpha', project: 'alpha', source: 'terminal', rc_id: 'session_01B', state: 'needs', since: now - 2 * MIN, waiting: 'input needed' }),
    s({ title: 'Fix login', name: 'fix-login', project: 'pb-bot', ws: 'src', source: 'podbay', rc_id: 'session_01C', state: 'needs',
        since: now - 5 * MIN, waiting: 'permission prompt', reason: { kind: 'permission', detail: '' } }),
  ];
  const counts = { needs: 2, busy: 0, idle: 1, stopped: 0, finished: 0 };
  const snap = { ok: true, error: null, read_at: now, dnd_until: null, counts, sessions, stopped: [], machines: [{ name: 'M0', online: true }] };
  window.__listeners = [];
  window.__snap = () => JSON.parse(JSON.stringify(snap));
  window.__TAURI__ = {
    core: { invoke: (cmd) => Promise.resolve(cmd === 'snapshot' ? window.__snap() : null) },
    event: { listen: (name, cb) => { window.__listeners.push(cb); return Promise.resolve(() => {}); } },
  };
})();
"""

PROBE = r"""
(async () => {
  const out = { errors: [] };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const clean = (t) => (t || '').replace(/\s+/g, ' ').trim();
  try {
    await sleep(600);
    if (window.__PROBE_MODE === 'popover') {
      out.rows = [...document.querySelectorAll('#list button')].map((b) => clean(b.innerText));
    } else {
      const grab = (sel) => [...document.querySelectorAll(sel)].map((r) => ({ key: r.dataset.key, text: clean(r.innerText), flash: r.classList.contains('flash') }));
      out.board = grab('#view li.row');
      // The same read again: no state changed, so no row may flash.
      for (const cb of window.__listeners) cb({ payload: window.__snap() });
      await sleep(300);
      out.again = grab('#view li.row');
      document.getElementById('lay-list').click(); await sleep(200);
      out.list = grab('#view .lrow');
      for (const cb of window.__listeners) cb({ payload: window.__snap() });
      await sleep(300);
      out.listAgain = grab('#view .lrow');
    }
  } catch (e) { out.errors.push(String((e && e.stack) || e)); }
  const pre = document.createElement('pre'); pre.id = '__probe'; pre.textContent = JSON.stringify(out);
  document.body.append(pre);
})();
"""

_cache = {}


def probe(page, mode):
    if page in _cache:
        return _cache[page]
    d = Path(tempfile.mkdtemp(prefix="pod-board-158-"))
    try:
        ui = d / "ui"
        shutil.copytree(UI, ui)
        src = (ui / page).read_text(encoding="utf-8")
        src = src.replace("<head>", f"<head><script>{MOCK}</script>", 1)
        src = src.replace("</body>", f"<script>window.__PROBE_MODE={json.dumps(mode)};</script><script>{PROBE}</script></body>")
        (ui / page).write_text(src, encoding="utf-8")
        cmd = [CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--allow-file-access-from-files", "--window-size=1180,900",
               "--virtual-time-budget=20000", f"--user-data-dir={d / 'profile'}", "--dump-dom", (ui / page).as_uri()]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    finally:
        shutil.rmtree(d, ignore_errors=True)
    m = re.search(r'<pre id="__probe">(.*?)</pre>', r.stdout, flags=re.S)
    if not m:
        raise AssertionError(f"probe produced no result (chrome exit {r.returncode}): {r.stderr[-300:]}")
    out = json.loads(html.unescape(m.group(1)))
    if out["errors"]:
        raise AssertionError(f"page script failed with the mock data: {out['errors']}")
    _cache[page] = out
    return out


@unittest.skipUnless(CHROME, "no google-chrome/chromium on this machine: headless page checks skipped")
class RowKeys(unittest.TestCase):
    # Bug 6: unnamed terminal sessions in one project shared the row key `machine|title`.
    def test_each_card_and_list_row_has_its_own_key(self):
        out = probe("index.html", "board")
        for name in ("board", "list"):
            rows = [r for r in out[name] if r["text"].startswith("alpha")]
            self.assertEqual(len(rows), 2, f"{name}: both alpha sessions show: {out[name]}")
            keys = [r["key"] for r in out[name]]
            self.assertEqual(len(keys), len(set(keys)), f"{name}: row keys collide: {keys}")

    def test_a_read_with_no_change_flashes_no_row(self):
        out = probe("index.html", "board")
        for name in ("again", "listAgain"):
            self.assertTrue(out[name], name)
            self.assertEqual([r["text"] for r in out[name] if r["flash"]], [], f"{name}: a row flashed without a state change")


@unittest.skipUnless(CHROME, "no google-chrome/chromium on this machine: headless page checks skipped")
class WaitingTag(unittest.TestCase):
    # Bug 5: the board, list and peek show the waiting tag next to a known reason; the popover hid it. One rule now:
    # the tag shows whenever the session names what it waits for (the #137 acceptance test keeps it on the card).
    def test_the_popover_shows_the_waiting_tag_next_to_a_known_reason_as_the_board_does(self):
        card = next(r for r in probe("index.html", "board")["board"] if r["text"].startswith("Fix login"))
        self.assertIn("Permission", card["text"])
        self.assertIn("permission prompt", card["text"])
        row = next(t for t in probe("popover.html", "popover")["rows"] if t.startswith("Fix login"))
        self.assertIn("Permission", row)
        self.assertIn("permission prompt", row)

    def test_with_no_known_reason_every_surface_shows_the_tag(self):
        card = next(r for r in probe("index.html", "board")["board"] if r["text"].startswith("alpha") and "input needed" in r["text"])
        self.assertNotRegex(card["text"], r"Permission|Question|Plan approval")
        rows = [t for t in probe("popover.html", "popover")["rows"] if t.startswith("alpha")]
        self.assertTrue(any("input needed" in t for t in rows), rows)


if __name__ == "__main__":
    unittest.main(verbosity=2)
