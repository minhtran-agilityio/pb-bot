"""Page tests for issue #159, item 4: the window layout with long text.

Run: python3 -I apps/pod-board/tests/pages/test_pages_159.py

A headless Chrome loads a copy of the window page with a mock Tauri runtime (long titles and branch names, a tall Working
card, a Stopped strip) at the app's default size, 1180 x 720, and dumps what the probe measured. Skipped, loudly, when
no Chrome/Chromium binary exists. Nothing in the app is changed.
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
MAC_CHROME = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
CHROME = next((p for p in (shutil.which(n) for n in ("google-chrome", "chromium", "chromium-browser", "google-chrome-stable")) if p),
              str(MAC_CHROME) if MAC_CHROME.is_file() else None)

LONG_BRANCH = "feature/pod-shape-grill-with-a-very-long-branch-name"
MOCK = r"""
(() => {
  const now = Date.now(), MIN = 60000;
  const s = (o) => Object.assign({ ws: 'src', machine: 'M0', waiting: '', what: '', branch: '', links: [], prs: 0, queued: 0, name: '',
    source: 'podbay', project: 'pb-bot', rc_id: '' }, o);
  const agents = (n) => ({ running: n, done: 0, list: Array.from({ length: n }, (_, i) => ({ type: 'implementer',
    task: 'Write the parser for the long configuration file format, part ' + i, since: now - i * MIN, depth: 0, done: false })) });
  const sessions = [
    s({ title: 'PB Test Session With A Rather Long Title', state: 'idle', since: now - 30 * MIN, branch: 'LONG_BRANCH' }),
    s({ title: 'PB Test Idle Two', state: 'idle', since: now - 40 * MIN }),
    s({ title: 'PB Test Working Card With Long Title', state: 'busy', since: now - 3 * MIN, branch: 'LONG_BRANCH', agents: agents(5),
        links: [{ kind: 'pr', url: 'https://example.invalid/1', title: 'PR #1' }, { kind: 'issue', url: 'https://example.invalid/2', title: 'Issue #2' }] }),
    s({ title: 'PB Test Working Two', state: 'busy', since: now - 3 * MIN, agents: agents(4),
        what: 'A long description of what this session does right now, long enough to need more than one line in the card.' }),
    s({ title: 'PB Test Waiting With A Long Title Too', state: 'needs', since: now - 5 * MIN, waiting: 'permission prompt',
        reason: { kind: 'permission', detail: '' }, branch: 'LONG_BRANCH' }),
  ];
  const stopped = Array.from({ length: 6 }, (_, i) => ({ project: 'pb-bot', ws: 'src-0' + i, machine: 'M0', dirty: false, source: 'podbay' }));
  const counts = { needs: 1, busy: 2, idle: 2, stopped: stopped.length, finished: 0 };
  const snap = { ok: true, error: null, read_at: now, dnd_until: null, counts, sessions, stopped, machines: [{ name: 'M0', online: true }] };
  window.__snap = () => JSON.parse(JSON.stringify(snap));
  window.__TAURI__ = {
    core: { invoke: (cmd) => Promise.resolve(cmd === 'snapshot' ? window.__snap() : null) },
    event: { listen: () => Promise.resolve(() => {}) },
  };
})();
""".replace("LONG_BRANCH", LONG_BRANCH)

PROBE = r"""
(async () => {
  const out = { errors: [] };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const box = (e) => { const r = e.getBoundingClientRect(); return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, width: r.width, height: r.height }; };
  const lines = (e) => Math.round(e.getBoundingClientRect().height / parseFloat(getComputedStyle(e).lineHeight));
  const overlap = (a, b) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
  const cards = () => [...document.querySelectorAll('#view li.row')];
  const shown = (t) => { const cs = getComputedStyle(t), r = t.getBoundingClientRect(); return cs.display !== 'none' && cs.visibility !== 'hidden' && cs.opacity !== '0' && r.width > 0 && r.height > 0; };
  try {
    await sleep(800);
    const measure = () => cards().map((li) => {
      const t = li.querySelector('.title'), f = li.querySelector('.what.fallback'), cs = getComputedStyle(t), inner = li.clientWidth - 40;
      return { title: t.textContent, lane: li.closest('.lane') ? li.closest('.lane').className : '', titleWidth: t.getBoundingClientRect().width,
        cardInner: inner, titleLines: lines(t), peekApart: (() => { const tm = li.querySelector('.wait, .when'), pk = li.querySelector('.peekbtn');
          return !!(tm && pk) && Math.abs((pk.getBoundingClientRect().top + pk.getBoundingClientRect().bottom) / 2 - (tm.getBoundingClientRect().top + tm.getBoundingClientRect().bottom) / 2) > 8; })(), titleEllipsis: cs.textOverflow === 'ellipsis' && cs.whiteSpace === 'nowrap',
        branch: f ? { text: f.textContent, lines: lines(f), ellipsis: getComputedStyle(f).textOverflow === 'ellipsis' } : null };
    });
    out.cards = measure();
    // The Stopped strip may cover no card, at the top of the panel and scrolled to the end.
    const main = document.querySelector('main'), strip = document.querySelector('.strip');
    out.scrolls = main.scrollHeight > main.clientHeight;
    out.covered = [];
    for (const at of ['top', 'end']) {
      main.scrollTop = at === 'top' ? 0 : main.scrollHeight; await sleep(150);
      const sb = box(strip), mb = box(main);
      for (const li of cards()) {
        const cb = box(li);
        // Only the part of the card inside the panel's view counts.
        const seen = { ...cb, top: Math.max(cb.top, mb.top), bottom: Math.min(cb.bottom, mb.bottom) };
        if (seen.bottom > seen.top && overlap(seen, sb)) out.covered.push({ at, card: li.querySelector('.title').textContent });
      }
    }
    main.scrollTop = 0;
    // The lanes are wider with the sidebar folded: a title row that fits its time may still not fit the peek button.
    document.getElementById('sidebar').click(); await sleep(300);
    out.cardsCollapsed = measure();
  } catch (e) { out.errors.push(String((e && e.stack) || e)); }
  const pre = document.createElement('pre'); pre.id = '__probe'; pre.textContent = JSON.stringify(out);
  document.body.append(pre);
})();
"""

_cache = {}


def probe():
    if "out" in _cache:
        return _cache["out"]
    d = Path(tempfile.mkdtemp(prefix="pod-board-159-"))
    try:
        ui = d / "ui"
        shutil.copytree(UI, ui)
        src = (ui / "index.html").read_text(encoding="utf-8")
        src = src.replace("<head>", f"<head><script>{MOCK}</script>", 1)
        src = src.replace("</body>", f"<script>{PROBE}</script></body>")
        (ui / "index.html").write_text(src, encoding="utf-8")
        cmd = [CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--allow-file-access-from-files", "--window-size=1180,720",
               "--virtual-time-budget=30000", f"--user-data-dir={d / 'profile'}", "--dump-dom", (ui / "index.html").as_uri()]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    finally:
        shutil.rmtree(d, ignore_errors=True)
    m = re.search(r'<pre id="__probe">(.*?)</pre>', r.stdout, flags=re.S)
    if not m:
        raise AssertionError(f"probe produced no result (chrome exit {r.returncode}): {r.stderr[-300:]}")
    out = json.loads(html.unescape(m.group(1)))
    if out["errors"]:
        raise AssertionError(f"page script failed with the mock data: {out['errors']}")
    _cache["out"] = out
    return out


@unittest.skipUnless(CHROME, "no google-chrome/chromium on this machine: headless page checks skipped")
class Layout(unittest.TestCase):
    def test_titles_keep_to_one_line_and_get_most_of_the_card(self):
        # Item 4: in the narrow Idle lane the title was cut to "PB Test ..." (60 px of a 167 px card).
        for c in probe()["cards"] + probe()["cardsCollapsed"]:
            self.assertEqual(c["titleLines"], 1, c)
            self.assertTrue(c["titleEllipsis"], f"title not cut with an ellipsis: {c}")
            self.assertGreaterEqual(c["titleWidth"], 0.7 * c["cardInner"], f"title squeezed: {c}")

    def test_the_peek_button_stays_beside_the_time(self):
        # A wrapped title row may not leave the (hidden until hover) peek button alone on an empty line.
        for c in probe()["cards"] + probe()["cardsCollapsed"]:
            self.assertFalse(c["peekApart"], c)

    def test_branch_names_keep_to_one_line_with_an_ellipsis(self):
        branches = [c["branch"] for c in probe()["cards"] if c["branch"]]
        self.assertTrue(branches, "the mock shows at least one branch line")
        for b in branches:
            self.assertEqual(b["lines"], 1, b)
            self.assertTrue(b["ellipsis"], b)

    def test_the_stopped_strip_covers_no_card(self):
        out = probe()
        self.assertTrue(out["scrolls"], "the mock must be taller than the panel")
        self.assertEqual(out["covered"], [], "cards under the Stopped strip")


if __name__ == "__main__":
    unittest.main(verbosity=2)
