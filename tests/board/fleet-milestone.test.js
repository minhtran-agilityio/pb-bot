#!/usr/bin/env node
'use strict';
// Unit tests for the milestone fields of /fleet.json (#16): present, absent and wrong-type values, through fleetOf.
// Run: node tests/board/fleet-milestone.test.js
const assert = require('assert');
const path = require('path');
const { fleetOf } = require(path.join(__dirname, '..', '..', 'plugins', 'pd-bot', 'board', 'board.js'));
const led = extra => fleetOf({ run: 'r', base: 'b', tickets: { 1: { state: 'done' } }, ...extra });
const ROUNDS = [{ round: 1, failed: ['S1'], tickets: [4, 5] }, { round: 2, failed: [], tickets: [6] }];
const cases = [
  ['present', () => {
    const j = led({ scenarios: 'https://x/y', feature_pr: 9, test_rounds: ROUNDS });
    assert.deepStrictEqual([j.scenarios, j.feature_pr, j.test_rounds], ['https://x/y', 9, ROUNDS]);
  }],
  ['absent', () => {
    const j = led({});
    assert.deepStrictEqual([j.scenarios, j.feature_pr, j.test_rounds], [null, null, []]);
  }],
  ['wrong types are absent, no throw', () => {
    for (const v of [42, 'x', true, {}, [1], null]) {
      const j = led({ scenarios: v === 'x' ? 42 : v, feature_pr: v === 42 ? 'x' : v, test_rounds: v });
      assert.strictEqual(j.feature_pr, null);
      assert.ok(j.scenarios === null || typeof j.scenarios === 'string');
      assert.ok(Array.isArray(j.test_rounds));
      assert.strictEqual(j.tickets.length, 1);
    }
    assert.strictEqual(led({ scenarios: 5 }).scenarios, null);
    assert.strictEqual(led({ feature_pr: '7' }).feature_pr, null);
    assert.strictEqual(led({ feature_pr: NaN }).feature_pr, null);
  }],
  ['blank scenarios (only whitespace) is absent', () => {
    for (const v of ['', ' ', '   ', '\t\n ']) assert.strictEqual(led({ scenarios: v }).scenarios, null);
    assert.strictEqual(led({ scenarios: ' https://x/y ' }).scenarios, ' https://x/y ');
  }],
  ['malformed test_rounds entries are dropped, bad members filtered', () => {
    const j = led({ test_rounds: [null, 3, [], { failed: [] }, { round: 7, failed: 'x', tickets: [1, {}, 'a', null] }] });
    assert.deepStrictEqual(j.test_rounds, [{ round: 7, failed: [], tickets: [1, 'a'] }]);
  }],
];
let failed = 0;
for (const [n, f] of cases) { try { f(); console.log(`PASS ${n}`); } catch (e) { failed++; console.log(`FAIL ${n}\n  ${e.message.split('\n')[0]}`); } }
console.log(failed ? `fleet-milestone: ${failed} failed` : `fleet-milestone: ${cases.length} passed`);
process.exit(failed ? 1 : 0);
