#!/usr/bin/env node
'use strict';
// Acceptance tests for #16 "board /fleet page: show the milestone fields of the ledger": the page criteria
// (AC4, AC5, AC6, AC7), in a real browser. They need Playwright; without it the file prints "skip" and exits 0, like
// tests/board/fleet-page.test.js. When Playwright's own Chromium is missing, the installed Google Chrome is used
// (channel "chrome"); with neither, the file prints "skip" and exits 0.
// Each case starts its own board through the CLI (`--port <free port> --ledger <file>`) with HOME set to a fresh
// temp dir, and stops it afterwards. Prints one PASS/FAIL line per case and exits 1 on any failure.
// Run: NODE_PATH=<dir with playwright> node tests/acceptance/fleet-milestone-page.test.js
//
// Selectors: the page markup is not known, so no CSS class is used. Text is found by what a viewer sees
// (innerText); a round "line" is the smallest element whose text holds that round's number, failed count and
// ticket numbers, and no other round's tickets.
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const net = require('net');
const path = require('path');
const http = require('http');
const { spawn } = require('child_process');

let chromium;
try { ({ chromium } = require('playwright')); } catch (e) { console.log('acceptance 16 (page): skip — no playwright'); process.exit(0); }

const root = path.join(__dirname, '..', '..');
const BOARD = path.join(root, 'plugins', 'pd-bot', 'board', 'board.js');
const cases = [];
const test = (name, fn) => cases.push([name, fn]);
const tmp = p => fs.mkdtempSync(path.join(os.tmpdir(), `acc16p-${p}-`));
const sleep = ms => new Promise(r => setTimeout(r, ms));
const get = (port, p) => new Promise((res, rej) => http.get({ host: '127.0.0.1', port, path: p }, r => {
  let b = ''; r.on('data', c => { b += c; }); r.on('end', () => res({ status: r.statusCode, body: b }));
}).on('error', rej));
const freePort = () => new Promise((res, rej) => {
  const s = net.createServer(); s.unref(); s.on('error', rej);
  s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => res(port)); });
});
async function board(args) {
  const port = await freePort();
  const env = { ...process.env, HOME: tmp('home') };
  delete env.CLAUDE_CONFIG_DIR;
  const child = spawn(process.execPath, [BOARD, '--port', String(port), ...args], { cwd: tmp('cwd'), env, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = ''; child.stdout.on('data', c => { out += c; }); child.stderr.on('data', c => { out += c; });
  let exited = null; child.on('exit', code => { exited = code; });
  for (let i = 0; i < 100; i++) {
    if (exited !== null) throw new Error(`board exited with code ${exited} before it answered: ${out.trim().split('\n')[0]}`);
    try { if ((await get(port, '/healthz')).status === 200) return { port, stop: () => child.kill() }; } catch (e) { /* not up yet */ }
    await sleep(50);
  }
  child.kill();
  throw new Error(`board did not answer on port ${port} within 5 s`);
}

// Ledger format: plugins/pd-bot/skills/pd-orchestrate/references/protocol.md, section "Ledger".
const TICKETS = {
  '14': { state: 'done', kind: 'feature', depth: 'M', worker: { session: 'w-fourteen-sess' }, reviewer: { session: 'r-fourteen-sess' }, head: 'h14', pr: null },
  '15': { state: 'in_review', kind: 'bug', depth: 'S', worker: { session: 'w-fifteen-sess' }, reviewer: null, head: 'h15', pr: null },
};
const ledger = extra => ({ v: 1, run: 'm16ui', base: 'feat/milestone', repo: 'owner/name', tickets: TICKETS, ...extra });
function ledgerFile(obj) {
  const file = path.join(tmp('led'), 'specs', 'fleet', 'm16ui', 'ledger.json');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(obj, null, 2));
  return file;
}
const FEATURE_PR = 4321;
const ROUNDS = [
  { round: 7, failed: ['S-alpha', 'S-beta', 'S-gamma'], tickets: [501, 502] },
  { round: 8, failed: [], tickets: [503] },
];
const MILESTONE = { scenarios: 'https://example.test/owner/name/scenarios-m16.md', feature_pr: FEATURE_PR, test_rounds: ROUNDS };

async function openFleet(browser, file) {
  const b = await board(['--ledger', file]);
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  try {
    const resp = await page.goto(`http://127.0.0.1:${b.port}/fleet`);
    assert.strictEqual(resp.status(), 200, `GET /fleet status ${resp.status()}`);
    await page.waitForFunction(w => document.body && document.body.innerText.includes(w), TICKETS['14'].worker.session, { timeout: 5000 })
      .catch(() => { throw new Error('the /fleet page never showed the ticket rows'); });
  } catch (e) { await page.close(); b.stop(); throw e; }
  return { page, stop: async () => { await page.close(); b.stop(); } };
}
const waitText = (page, s, what) => page.waitForFunction(t => document.body.innerText.includes(t), s, { timeout: 5000 })
  .catch(() => { throw new Error(`the /fleet page never showed ${what} (${JSON.stringify(s)})`); });
const tok = (text, n) => new RegExp(`(^|[^0-9A-Za-z])${n}([^0-9A-Za-z]|$)`).test(text);

// In the page: the smallest element whose text holds every token in `want` and none of `not`.
function lineOf({ want, not }) {
  const tok = (text, n) => new RegExp(`(^|[^0-9A-Za-z])${n}([^0-9A-Za-z]|$)`).test(text);
  let best = null;
  for (const el of document.body.querySelectorAll('*')) {
    const t = el.innerText || '';
    if (!want.every(w => tok(t, w)) || not.some(n => tok(t, n))) continue;
    if (!best || best.contains(el)) best = el;
  }
  return best ? best.innerText : null;
}

// "AC4: The /fleet page shows the feature PR number, the scenarios value, and one line per test round with the round
// number, the failed count (length of failed) and the ticket numbers, when present."
test('AC4: the page shows the feature PR number and the scenarios value', async browser => {
  const f = await openFleet(browser, ledgerFile(ledger(MILESTONE)));
  try {
    await waitText(f.page, String(FEATURE_PR), 'the feature PR number');
    await waitText(f.page, MILESTONE.scenarios, 'the scenarios value');
  } finally { await f.stop(); }
});
test('AC4: one line per test round with round number, failed count and ticket numbers', async browser => {
  const f = await openFleet(browser, ledgerFile(ledger(MILESTONE)));
  try {
    await waitText(f.page, '503', 'the ticket numbers of the last round');
    // Round 7: failed count 3, tickets 501 and 502; the line must not also hold round 8's ticket.
    const l7 = await f.page.evaluate(lineOf, { want: ['501', '502'], not: ['503'] });
    assert.ok(l7, 'no element shows the tickets 501 and 502 of round 7 apart from round 8');
    const rest7 = l7.split('501').join(' ').split('502').join(' ');
    assert.ok(tok(rest7, '7'), `the round 7 line does not show the round number 7: ${JSON.stringify(l7)}`);
    assert.ok(tok(rest7, '3'), `the round 7 line does not show the failed count 3: ${JSON.stringify(l7)}`);
    // Round 8: failed count 0, ticket 503; the line must not also hold round 7's tickets.
    const l8 = await f.page.evaluate(lineOf, { want: ['503'], not: ['501', '502'] });
    assert.ok(l8, 'no element shows the ticket 503 of round 8 apart from round 7');
    const rest8 = l8.split('503').join(' ');
    assert.ok(tok(rest8, '8'), `the round 8 line does not show the round number 8: ${JSON.stringify(l8)}`);
    assert.ok(tok(rest8, '0'), `the round 8 line does not show the failed count 0: ${JSON.stringify(l8)}`);
  } finally { await f.stop(); }
});

// "AC5: The scenarios value renders as a link (target _blank, rel noopener noreferrer) only when it starts with
// http:// or https://; otherwise it is plain text."
const linksTo = (page, s) => page.evaluate(v => [...document.querySelectorAll('a')]
  .filter(a => (a.getAttribute('href') || '').includes(v) || (a.innerText || '').includes(v))
  .map(a => ({ href: a.getAttribute('href'), target: a.getAttribute('target'), rel: a.getAttribute('rel') || '' })), s);
for (const url of ['https://example.test/owner/name/scenarios-m16.md', 'http://example.test/plain-http/scenarios.md']) {
  test(`AC5: a scenarios value starting with ${url.split(':')[0]}:// is a link with target _blank and rel noopener noreferrer`, async browser => {
    const f = await openFleet(browser, ledgerFile(ledger({ ...MILESTONE, scenarios: url })));
    try {
      await f.page.waitForFunction(v => [...document.querySelectorAll('a')].some(a => a.getAttribute('href') === v), url, { timeout: 5000 })
        .catch(() => { throw new Error(`no link with href ${url} on the page`); });
      const ls = (await linksTo(f.page, url)).filter(l => l.href === url);
      for (const l of ls) {
        assert.strictEqual(l.target, '_blank', `the scenarios link has target ${l.target}`);
        const rel = l.rel.toLowerCase().split(/\s+/);
        assert.ok(rel.includes('noopener') && rel.includes('noreferrer'), `the scenarios link has rel "${l.rel}"`);
      }
    } finally { await f.stop(); }
  });
}
for (const plain of ['specs/m16ui/scenarios.md', 'javascript:alert(16)', 'ftp://example.test/scenarios.md']) {
  test(`AC5: a scenarios value ${JSON.stringify(plain)} is plain text, not a link`, async browser => {
    const f = await openFleet(browser, ledgerFile(ledger({ ...MILESTONE, scenarios: plain })));
    try {
      await waitText(f.page, plain.trim(), 'the scenarios value as text');
      const ls = await linksTo(f.page, plain.trim());
      assert.deepStrictEqual(ls, [], `the scenarios value ${JSON.stringify(plain)} is a link: ${JSON.stringify(ls)}`);
    } finally { await f.stop(); }
  });
}

// "AC6: With all three fields absent, the page shows no milestone section or empty labels (nothing extra)."
// First the page shows a milestone ledger (so the milestone text is known to render), then the ledger is rewritten
// without the three fields and the page, after its own refresh, must show none of that text and no milestone label.
const LABELS = /\bscenarios?\b|\bfeature[ _-]?pr\b|\btest[ _-]?rounds?\b|\brounds?\b/i;
test('AC6: with all three fields absent the page shows no milestone section and no empty labels', async browser => {
  const file = ledgerFile(ledger(MILESTONE));
  const f = await openFleet(browser, file);
  try {
    await waitText(f.page, String(FEATURE_PR), 'the feature PR number of the milestone ledger');
    fs.writeFileSync(file, JSON.stringify(ledger({}), null, 2));
    await f.page.waitForFunction(v => !document.body.innerText.includes(v), String(FEATURE_PR), { timeout: 8000 })
      .catch(() => { throw new Error('the milestone section did not go away after the fields were removed from the ledger'); });
    await sleep(500);
    const text = await f.page.evaluate(() => document.body.innerText);
    assert.ok(text.includes(TICKETS['14'].worker.session), 'the ticket rows are gone');
    for (const s of [MILESTONE.scenarios, '501', '502', '503']) assert.ok(!text.includes(s), `the page still shows ${s}`);
    const m = text.match(LABELS);
    assert.ok(!m, `the page shows a milestone label with all three fields absent: ${JSON.stringify(m && m[0])}`);
  } finally { await f.stop(); }
});

// "AC7: Any ledger text (scenarios, round values, failed ids, ticket numbers) is set with textContent or equivalent,
// so markup like <img onerror> in the ledger shows as text and no element is created from it."
test('AC7: markup in scenarios, failed ids and ticket numbers shows as text and creates no element', async browser => {
  const SCEN = '<img id="pwn16s" src="x" onerror="window.__pwn16=1">';
  const xss = {
    scenarios: SCEN,
    feature_pr: FEATURE_PR,
    test_rounds: [
      { round: 1, failed: ['<img id="pwn16f" src="x" onerror="window.__pwn16=2">', '<b id="pwn16b">bold</b>'], tickets: ['<i id="pwn16t">t</i>', 601] },
    ],
  };
  const f = await openFleet(browser, ledgerFile(ledger(xss)));
  try {
    await waitText(f.page, SCEN, 'the scenarios markup as text');
    await sleep(500);
    const r = await f.page.evaluate(() => ({
      ids: ['pwn16s', 'pwn16f', 'pwn16b', 'pwn16t'].filter(id => document.getElementById(id)),
      imgs: [...document.querySelectorAll('img')].map(i => i.getAttribute('src')),
      flag: window.__pwn16,
    }));
    assert.deepStrictEqual(r.ids, [], `elements were created from ledger text: ${r.ids.join(', ')}`);
    assert.ok(!r.imgs.includes('x'), 'an <img src="x"> was created from ledger text');
    assert.strictEqual(r.flag, undefined, 'an onerror handler from the ledger ran');
  } finally { await f.stop(); }
});
test('AC7: an http(s) scenarios value with a quote and markup creates no element', async browser => {
  const SCEN = 'https://example.test/a"><img id="pwn16q" src="x" onerror="window.__pwn16=3">';
  const f = await openFleet(browser, ledgerFile(ledger({ ...MILESTONE, scenarios: SCEN })));
  try {
    await waitText(f.page, String(FEATURE_PR), 'the milestone section');
    await sleep(500);
    const r = await f.page.evaluate(() => ({ el: !!document.getElementById('pwn16q'), flag: window.__pwn16 }));
    assert.strictEqual(r.el, false, 'an element was created from the scenarios value');
    assert.strictEqual(r.flag, undefined, 'an onerror handler from the ledger ran');
  } finally { await f.stop(); }
});

(async () => {
  let browser;
  try { browser = await chromium.launch(); } catch (e) {
    try { browser = await chromium.launch({ channel: 'chrome' }); } catch (e2) {
      console.log('acceptance 16 (page): skip — no Chromium for Playwright and no Google Chrome'); process.exit(0);
    }
  }
  let failed = 0;
  try {
    for (const [name, fn] of cases) {
      try { await fn(browser); console.log(`PASS ${name}`); } catch (e) { failed++; console.log(`FAIL ${name}\n  ${e && e.message ? e.message.split('\n')[0] : e}`); }
    }
  } finally { await browser.close(); }
  console.log(failed ? `acceptance 16 (page): ${failed} of ${cases.length} failed` : `acceptance 16 (page): ${cases.length} passed`);
  process.exit(failed ? 1 : 0);
})();
