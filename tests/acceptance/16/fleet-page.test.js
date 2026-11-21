#!/usr/bin/env node
'use strict';
// Acceptance tests for #16 "board /fleet page: show the milestone fields of the ledger": the page criteria, in a real browser.
// Needs Playwright with a Chromium; without it the file prints "skip" and exits 0 (like tests/board/fleet-page.test.js).
// Selectors never use CSS classes: the page is judged by its visible text, links by role and href.
// Run: NODE_PATH=<dir with playwright> node tests/acceptance/16/fleet-page.test.js
const assert = require('assert');
let chromium;
try { ({ chromium } = require('playwright')); } catch (e) { console.log('acceptance 16 (page): skip - no playwright'); process.exit(0); }
const { board, ledger, ledgerFile, run } = require('./harness');
const cases = [];
const test = (name, fn) => cases.push([name, fn]);

async function open(browser, extra) {
  const b = await board(['--ledger', ledgerFile(ledger(extra))]);
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await page.goto(`http://127.0.0.1:${b.port}/fleet`);
  await page.waitForFunction(() => document.body && document.body.innerText.includes('w-five-sess'), null, { timeout: 5000 })
    .catch(() => { throw new Error('the /fleet page never showed the ticket row'); });
  return { page, errors, stop: async () => { await page.close(); b.stop(); } };
}
const text = page => page.evaluate(() => document.body.innerText);
const ROUNDS = [{ round: 3, failed: ['S1', 'S4', 'S9'], tickets: [201, 202] }, { round: 4, failed: [], tickets: [203] }];
const MILESTONE = /scenario|feature pr|test round|round/i;

// "The `/fleet` page shows the feature PR number, the scenario link and one line per test round (round, failed count, ticket numbers) when present"
test('AC2: the page shows the feature PR number and the scenario link when present', async br => {
  const f = await open(br, { scenarios: 'https://example.com/scen/16', feature_pr: 7731 });
  try {
    assert.ok((await text(f.page)).includes('7731'), 'feature PR number 7731 not on the page');
    const hrefs = await f.page.evaluate(() => [...document.querySelectorAll('a[href]')].map(a => a.getAttribute('href')));
    assert.ok(hrefs.includes('https://example.com/scen/16'), `no link to the scenarios (links: ${hrefs.join(' ') || 'none'})`);
  } finally { await f.stop(); }
});
test('AC2: the page shows one line per test round with round, failed count and ticket numbers', async br => {
  const f = await open(br, { test_rounds: ROUNDS });
  try {
    const lines = (await text(f.page)).split('\n');
    const l3 = lines.find(l => /\b3\b/.test(l) && l.includes('201') && l.includes('202'));
    assert.ok(l3, 'no line names round 3 with tickets 201 and 202');
    assert.ok(/\b3\b/.test(l3.replace(/201|202/g, '')) && l3.match(/\b3\b/g).length >= 2, `line for round 3 does not show both the round and the failed count 3: "${l3}"`);
    const l4 = lines.find(l => /\b4\b/.test(l) && l.includes('203'));
    assert.ok(l4, 'no line names round 4 with ticket 203');
    assert.ok(/\b0\b/.test(l4), `line for round 4 does not show the failed count 0: "${l4}"`);
    assert.notStrictEqual(l3, l4, 'rounds are not on separate lines');
  } finally { await f.stop(); }
});
// "... and nothing extra when absent."
test('AC2: with the fields absent, no milestone text appears', async br => {
  const f = await open(br, {});
  try {
    const t = await text(f.page);
    assert.ok(!MILESTONE.test(t), `milestone text on the page: ${(t.match(MILESTONE) || [])[0]}`);
    assert.strictEqual(await f.page.evaluate(() => document.querySelectorAll('a[href*="example.com"]').length), 0);
  } finally { await f.stop(); }
});
test('AC2: wrong-type values show no milestone text and the page does not crash', async br => {
  const f = await open(br, { scenarios: 42, feature_pr: 'seventy', test_rounds: 'x' });
  try {
    const t = await text(f.page);
    assert.ok(!MILESTONE.test(t), `milestone text on the page: ${(t.match(MILESTONE) || [])[0]}`);
    assert.ok(!t.includes('seventy'), 'a wrong-type feature_pr was shown');
    assert.deepStrictEqual(f.errors, []);
  } finally { await f.stop(); }
});
// "Any text from the ledger is escaped on the page."
test('AC3: markup in ledger text is shown as text and no element is injected', async br => {
  const X = '<img id="inj16" src=x onerror="window.__pwn16=1">';
  const f = await open(br, { scenarios: `javascript:alert(1)//${X}`, feature_pr: 5, test_rounds: [{ round: X, failed: [X], tickets: [X, 9] }] });
  try {
    await f.page.waitForTimeout(300);
    assert.strictEqual(await f.page.evaluate(() => document.querySelectorAll('#inj16, img[src="x"]').length), 0, 'an element was injected');
    assert.strictEqual(await f.page.evaluate(() => window.__pwn16 || 0), 0, 'ledger script ran');
  } finally { await f.stop(); }
});
test('AC3: markup in a valid scenario link text is not injected', async br => {
  const X = '<b id="inj16b">bold</b>';
  const f = await open(br, { scenarios: `https://example.com/s?q=${encodeURIComponent('"')}${X}`, feature_pr: 5 });
  try {
    assert.strictEqual(await f.page.evaluate(() => document.querySelectorAll('#inj16b').length), 0, 'an element was injected');
  } finally { await f.stop(); }
});

(async () => {
  const browser = await chromium.launch();
  let failed;
  try { failed = await run(cases, 'page', browser); } finally { await browser.close(); }
  process.exit(failed ? 1 : 0);
})();
