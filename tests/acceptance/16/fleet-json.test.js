#!/usr/bin/env node
'use strict';
// Acceptance tests for #16 "board /fleet page: show the milestone fields of the ledger": the JSON criterion.
// Plain node, no browser. Each case starts its own board through the CLI on a free port with a fresh HOME.
// Run: node tests/acceptance/16/fleet-json.test.js
const assert = require('assert');
const { board, get, ledger, ledgerFile, run } = require('./harness');
const cases = [];
const test = (name, fn) => cases.push([name, fn]);

async function fleetJson(obj) {
  const b = await board(['--ledger', ledgerFile(obj)]);
  try {
    const r = await get(b.port, '/fleet.json');
    assert.strictEqual(r.status, 200, `GET /fleet.json status ${r.status}`);
    return JSON.parse(r.body);
  } finally { b.stop(); }
}
const ROUNDS = [{ round: 1, failed: ['S1', 'S4'], tickets: [21, 22] }, { round: 2, failed: [], tickets: [23] }];
const absentScalar = v => v === null || v === undefined;
const absentRounds = v => v === null || (Array.isArray(v) && v.length === 0);

// "`/fleet.json` includes `scenarios`, `feature_pr` and `test_rounds` (null or [] when absent; a wrong type is treated as absent, never a crash)."
test('AC1: /fleet.json includes scenarios, feature_pr and test_rounds when present', async () => {
  const j = await fleetJson(ledger({ scenarios: 'https://example.com/scenarios/16', feature_pr: 77, test_rounds: ROUNDS }));
  assert.strictEqual(j.scenarios, 'https://example.com/scenarios/16');
  assert.strictEqual(j.feature_pr, 77);
  assert.deepStrictEqual(j.test_rounds, ROUNDS);
});
test('AC1: /fleet.json has null or [] for the three fields when absent', async () => {
  const j = await fleetJson(ledger({}));
  assert.ok('scenarios' in j && 'feature_pr' in j && 'test_rounds' in j, 'a field is missing from the JSON');
  assert.ok(absentScalar(j.scenarios) && j.scenarios !== undefined, `scenarios is ${JSON.stringify(j.scenarios)}`);
  assert.ok(j.feature_pr === null, `feature_pr is ${JSON.stringify(j.feature_pr)}`);
  assert.ok(absentRounds(j.test_rounds), `test_rounds is ${JSON.stringify(j.test_rounds)}`);
});
test('AC1: wrong-type values are treated as absent, the board answers 200 and keeps the tickets', async () => {
  const bad = [
    { scenarios: 42, feature_pr: 'seventy', test_rounds: 'x' },
    { scenarios: { a: 1 }, feature_pr: { n: 1 }, test_rounds: { round: 1 } },
    { scenarios: ['u'], feature_pr: [7], test_rounds: 5 },
    { scenarios: true, feature_pr: true, test_rounds: true },
  ];
  for (const extra of bad) {
    const j = await fleetJson(ledger(extra));
    assert.ok(j.scenarios === null, `scenarios ${JSON.stringify(extra.scenarios)} became ${JSON.stringify(j.scenarios)}`);
    assert.ok(j.feature_pr === null, `feature_pr ${JSON.stringify(extra.feature_pr)} became ${JSON.stringify(j.feature_pr)}`);
    assert.ok(absentRounds(j.test_rounds), `test_rounds ${JSON.stringify(extra.test_rounds)} became ${JSON.stringify(j.test_rounds)}`);
    assert.strictEqual(j.run, 'r16');
    assert.strictEqual(j.tickets.length, 1, 'the tickets are gone');
  }
});
test('AC1: one wrong-type field does not hide the valid ones', async () => {
  const j = await fleetJson(ledger({ scenarios: 5, feature_pr: 31, test_rounds: ROUNDS }));
  assert.ok(j.scenarios === null, `scenarios is ${JSON.stringify(j.scenarios)}`);
  assert.strictEqual(j.feature_pr, 31);
  assert.deepStrictEqual(j.test_rounds, ROUNDS);
});

(async () => { process.exit((await run(cases, 'json')) ? 1 : 0); })();
