#!/usr/bin/env node
'use strict';
// Acceptance tests for #16 "board /fleet page: show the milestone fields of the ledger": the /fleet.json, test and
// README criteria (AC1, AC2, AC3, AC8, AC9). Plain node, no browser.
// Each case starts its own board through the CLI
//   node plugins/pd-bot/board/board.js --port <free port> --ledger <file>
// on a free port, with HOME set to a fresh temp dir, and stops it afterwards.
// Prints one PASS/FAIL line per case and exits 1 on any failure.
// Run: node tests/acceptance/fleet-milestone-json.test.js
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const net = require('net');
const path = require('path');
const http = require('http');
const { spawn, spawnSync } = require('child_process');

const root = path.join(__dirname, '..', '..');
const BOARD = path.join(root, 'plugins', 'pd-bot', 'board', 'board.js');

const cases = [];
const test = (name, fn) => cases.push([name, fn]);
const tmp = p => fs.mkdtempSync(path.join(os.tmpdir(), `acc16-${p}-`));
const sleep = ms => new Promise(r => setTimeout(r, ms));
const get = (port, p) => new Promise((res, rej) => http.get({ host: '127.0.0.1', port, path: p }, r => {
  let b = ''; r.on('data', c => { b += c; }); r.on('end', () => res({ status: r.statusCode, type: r.headers['content-type'] || '', body: b }));
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
    if (exited !== null) throw new Error(`board exited with code ${exited} before it answered: ${out.trim()}`);
    try { const r = await get(port, '/healthz'); if (r.status === 200) return { port, child, stop: () => child.kill() }; } catch (e) { /* not up yet */ }
    await sleep(50);
  }
  child.kill();
  throw new Error(`board did not answer on port ${port} within 5 s: ${out.trim()}`);
}
async function withLedger(ledger, fn) {
  const file = path.join(tmp('led'), 'specs', 'fleet', 'm16', 'ledger.json');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, typeof ledger === 'string' ? ledger : JSON.stringify(ledger, null, 2));
  const b = await board(['--ledger', file]);
  try { return await fn(b.port, b); } finally { b.stop(); }
}
const fleet = async port => {
  const r = await get(port, '/fleet.json');
  let j = null;
  try { j = JSON.parse(r.body); } catch (e) { throw new Error(`/fleet.json body is not JSON (status ${r.status}): ${r.body.slice(0, 200)}`); }
  return { r, j };
};
const has = (o, k) => Object.prototype.hasOwnProperty.call(o, k);

// A ledger in the format of plugins/pd-bot/skills/pd-orchestrate/references/protocol.md, section "Ledger".
const TICKETS = {
  '14': { state: 'done', kind: 'feature', depth: 'M', worker: { session: 'w-14' }, reviewer: { session: 'r-14' }, head: 'h14', pr: 'https://github.com/owner/name/pull/14' },
  '15': { state: 'in_review', kind: 'bug', depth: 'S', worker: { session: 'w-15' }, reviewer: null, head: 'h15', pr: 15 },
};
const BASE = { v: 1, run: 'm16', base: 'feat/milestone', repo: 'owner/name', tickets: TICKETS };
const EXPECTED_TICKETS = [
  { id: '14', state: 'done', kind: 'feature', depth: 'M', worker: 'w-14', reviewer: 'r-14', head: 'h14', pr: 'https://github.com/owner/name/pull/14' },
  { id: '15', state: 'in_review', kind: 'bug', depth: 'S', worker: 'w-15', reviewer: null, head: 'h15', pr: 15 },
];
const KEYS = ['id', 'state', 'kind', 'depth', 'worker', 'reviewer', 'head', 'pr'];
const pick = t => Object.fromEntries(KEYS.map(k => [k, t[k]]));
const MILESTONE = {
  scenarios: 'https://github.com/owner/name/blob/feat/milestone/docs/scenarios.md',
  feature_pr: 77,
  test_rounds: [
    { round: 1, failed: ['S1', 'S3'], tickets: [14, 15] },
    { round: 2, failed: [], tickets: [15] },
  ],
};

// ---------- AC1 ----------
// "AC1: /fleet.json for a milestone ledger includes scenarios (string), feature_pr (integer) and test_rounds as
// [{round, failed:[...], tickets:[...]}], passed through from the ledger."
test('AC1: /fleet.json passes scenarios, feature_pr and test_rounds through from a milestone ledger', () => withLedger({ ...BASE, ...MILESTONE }, async port => {
  const { r, j } = await fleet(port);
  assert.strictEqual(r.status, 200, `status ${r.status}: ${r.body.slice(0, 200)}`);
  assert.strictEqual(j.scenarios, MILESTONE.scenarios);
  assert.strictEqual(j.feature_pr, 77);
  assert.deepStrictEqual(j.test_rounds, MILESTONE.test_rounds);
  // AC2: "the existing fields (run, base, tickets) are unchanged" also for a milestone ledger.
  assert.strictEqual(j.run, 'm16');
  assert.strictEqual(j.base, 'feat/milestone');
  assert.deepStrictEqual(j.tickets.map(pick), EXPECTED_TICKETS);
}));
test('AC1: a plain-text scenarios string is passed through unchanged', () => withLedger({ ...BASE, ...MILESTONE, scenarios: 'specs/m16/scenarios.md' }, async port => {
  const { r, j } = await fleet(port);
  assert.strictEqual(r.status, 200, `status ${r.status}`);
  assert.strictEqual(j.scenarios, 'specs/m16/scenarios.md');
}));

// ---------- AC2 ----------
// "AC2: /fleet.json for a ledger without these fields has scenarios: null, feature_pr: null, test_rounds: [] and the
// existing fields (run, base, tickets) are unchanged."
test('AC2: a ledger without the fields gives scenarios null, feature_pr null, test_rounds [], and run, base, tickets unchanged', () => withLedger(BASE, async port => {
  const { r, j } = await fleet(port);
  assert.strictEqual(r.status, 200, `status ${r.status}: ${r.body.slice(0, 200)}`);
  assert.ok(r.type.startsWith('application/json'), `content-type ${r.type}`);
  assert.strictEqual(j.run, 'm16');
  assert.strictEqual(j.base, 'feat/milestone');
  assert.deepStrictEqual(j.tickets.map(pick), EXPECTED_TICKETS);
  for (const k of ['scenarios', 'feature_pr', 'test_rounds']) assert.ok(has(j, k), `/fleet.json has no "${k}" key`);
  assert.strictEqual(j.scenarios, null);
  assert.strictEqual(j.feature_pr, null);
  assert.deepStrictEqual(j.test_rounds, []);
}));
// ---------- AC3 ----------
// "AC3: Wrong types never crash and are treated as absent: a non-string scenarios gives null; a feature_pr that is not
// a finite integer (string, float, object, NaN-like) gives null; a non-array test_rounds gives []; a round entry that
// is not an object or has no finite-number round is dropped; a non-array failed or tickets inside a kept round
// becomes []. The response stays 200."
// The ledger is written as raw text so that 1e400 (which JSON.parse reads as Infinity) can be used.
const raw = fields => JSON.stringify({ ...BASE, __F__: 0 }).replace('"__F__":0', fields);
async function expectAbsentAfter(fields, check) {
  await withLedger(raw(fields), async (port, b) => {
    const { r, j } = await fleet(port);
    assert.strictEqual(r.status, 200, `ledger ${fields}: status ${r.status}: ${r.body.slice(0, 200)}`);
    check(j, fields);
    assert.deepStrictEqual(j.tickets.map(pick), EXPECTED_TICKETS, `ledger ${fields}: tickets changed`);
    assert.strictEqual(b.child.exitCode, null, `ledger ${fields}: the board process exited`);
    assert.strictEqual((await get(port, '/healthz')).status, 200, `ledger ${fields}: /healthz no longer answers`);
  });
}
test('AC3: a non-string scenarios gives null (number, object, array, boolean, null)', async () => {
  for (const v of ['42', '{"url":"https://x"}', '["https://x"]', 'true', 'null']) {
    await expectAbsentAfter(`"scenarios":${v}`, (j, f) => assert.strictEqual(j.scenarios, null, `${f}: scenarios is ${JSON.stringify(j.scenarios)}`));
  }
});
test('AC3: a feature_pr that is not a finite integer gives null (string, float, object, NaN-like, Infinity, array, boolean)', async () => {
  for (const v of ['"77"', '7.5', '{"n":77}', '"NaN"', '1e400', '-1e400', '[77]', 'true', 'null']) {
    await expectAbsentAfter(`"feature_pr":${v}`, (j, f) => assert.strictEqual(j.feature_pr, null, `${f}: feature_pr is ${JSON.stringify(j.feature_pr)}`));
  }
});
test('AC3: a non-array test_rounds gives [] (string, object, number, null)', async () => {
  for (const v of ['"round 1"', '{"round":1,"failed":[],"tickets":[]}', '3', 'null', 'true']) {
    await expectAbsentAfter(`"test_rounds":${v}`, (j, f) => assert.deepStrictEqual(j.test_rounds, [], `${f}: test_rounds is ${JSON.stringify(j.test_rounds)}`));
  }
});
test('AC3: a round entry that is not an object or has no finite-number round is dropped', async () => {
  const rounds = '[null, 3, "r1", true, ["round",1], {"failed":["S1"],"tickets":[1]}, {"round":"2","failed":[],"tickets":[]},'
    + ' {"round":null,"failed":[],"tickets":[]}, {"round":1e400,"failed":[],"tickets":[]}, {"round":{"n":1},"failed":[],"tickets":[]},'
    + ' {"round":4,"failed":["S9"],"tickets":[14]}]';
  await expectAbsentAfter(`"test_rounds":${rounds}`, (j, f) => assert.deepStrictEqual(j.test_rounds, [{ round: 4, failed: ['S9'], tickets: [14] }],
    `test_rounds is ${JSON.stringify(j.test_rounds)}`));
});
test('AC3: a non-array failed or tickets inside a kept round becomes []', async () => {
  const rounds = '[{"round":1,"failed":"S1","tickets":{"14":true}}, {"round":2,"failed":null,"tickets":15}, {"round":3}, {"round":5,"failed":["S2"],"tickets":"14,15"}]';
  await expectAbsentAfter(`"test_rounds":${rounds}`, (j, f) => assert.deepStrictEqual(j.test_rounds, [
    { round: 1, failed: [], tickets: [] },
    { round: 2, failed: [], tickets: [] },
    { round: 3, failed: [], tickets: [] },
    { round: 5, failed: ['S2'], tickets: [] },
  ], `test_rounds is ${JSON.stringify(j.test_rounds)}`));
});
test('AC3: all three fields wrong at once still answers 200 with the absent values', async () => {
  await expectAbsentAfter('"scenarios":{"a":1},"feature_pr":"seventy","test_rounds":{"0":{"round":1}}', j => {
    assert.strictEqual(j.scenarios, null);
    assert.strictEqual(j.feature_pr, null);
    assert.deepStrictEqual(j.test_rounds, []);
  });
});

// ---------- AC8 ----------
// "AC8: Tests cover present, absent and wrong-type values: node tests/board/fleet-json.test.js and
// node tests/board/board.test.js exit 0; tests/board/fleet-page.test.js covers the page (skips without Playwright)."
const read = rel => fs.readFileSync(path.join(root, rel), 'utf8');
const runNode = (rel, env) => spawnSync(process.execPath, [path.join(root, rel)], { cwd: root, env, encoding: 'utf8', timeout: 300000 });
test('AC8: tests/board/fleet-json.test.js covers the three fields; it and tests/board/board.test.js exit 0', () => {
  const src = read('tests/board/fleet-json.test.js');
  for (const k of ['scenarios', 'feature_pr', 'test_rounds']) assert.ok(src.includes(k), `tests/board/fleet-json.test.js never mentions ${k}`);
  for (const rel of ['tests/board/fleet-json.test.js', 'tests/board/board.test.js']) {
    const r = runNode(rel, process.env);
    assert.strictEqual(r.status, 0, `node ${rel} exited ${r.status}: ${(r.stdout + r.stderr).split('\n').filter(l => /FAIL|Error/.test(l)).slice(0, 4).join(' | ')}`);
  }
});
test('AC8: tests/board/fleet-page.test.js covers the milestone fields on the page and skips without Playwright', () => {
  const src = read('tests/board/fleet-page.test.js');
  for (const k of ['scenarios', 'feature_pr', 'test_rounds']) assert.ok(src.includes(k), `tests/board/fleet-page.test.js never mentions ${k}`);
  const env = { ...process.env, NODE_PATH: tmp('nopw') };
  const r = runNode('tests/board/fleet-page.test.js', env);
  assert.strictEqual(r.status, 0, `without Playwright, node tests/board/fleet-page.test.js exited ${r.status}: ${(r.stdout + r.stderr).slice(0, 300)}`);
  assert.ok(/skip/i.test(r.stdout), `without Playwright, fleet-page.test.js did not say it skips: ${r.stdout.slice(0, 200)}`);
});

// ---------- AC9 ----------
// "AC9: plugins/pd-bot/board/README.md documents the three new /fleet.json fields, their null/[] defaults and the page lines."
test('AC9: the board README documents scenarios, feature_pr, test_rounds, their null/[] defaults and the round lines', () => {
  const readme = read('plugins/pd-bot/board/README.md');
  for (const k of ['scenarios', 'feature_pr', 'test_rounds']) assert.ok(readme.includes(k), `README does not mention ${k}`);
  assert.ok(readme.includes('[]'), 'README does not document the [] default of test_rounds');
  assert.ok(/null/.test(readme.slice(Math.max(0, readme.indexOf('feature_pr') - 400), readme.indexOf('feature_pr') + 400)),
    'README does not say null near feature_pr');
  assert.ok(/round/i.test(readme) && /failed/i.test(readme), 'README does not describe the per-round page lines (round, failed count)');
});

(async () => {
  let failed = 0;
  for (const [name, fn] of cases) {
    try { await fn(); console.log(`PASS ${name}`); } catch (e) { failed++; console.log(`FAIL ${name}\n  ${e && e.message ? e.message.split('\n').slice(0, 4).join('\n  ') : e}`); }
  }
  console.log(failed ? `acceptance 16 (json): ${failed} of ${cases.length} failed` : `acceptance 16 (json): ${cases.length} passed`);
  process.exit(failed ? 1 : 0);
})();
