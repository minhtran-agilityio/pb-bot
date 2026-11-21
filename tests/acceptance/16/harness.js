'use strict';
// Shared harness for the #16 acceptance tests: starts the board CLI on a free port with a fresh HOME.
const fs = require('fs');
const os = require('os');
const net = require('net');
const path = require('path');
const http = require('http');
const { spawn } = require('child_process');
const root = path.join(__dirname, '..', '..', '..');
const BOARD = path.join(root, 'plugins', 'pd-bot', 'board', 'board.js');
const tmp = p => fs.mkdtempSync(path.join(os.tmpdir(), `acc16-${p}-`));
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
  '5': { state: 'in_review', kind: 'feature', depth: 'M', worker: { session: 'w-five-sess' }, reviewer: { session: 'r-five-sess' }, head: 'h5', pr: 'https://github.com/owner/name/pull/5' },
};
const ledger = extra => ({ v: 1, run: 'r16', base: 'main', repo: 'owner/name', tickets: TICKETS, ...extra });
function ledgerFile(obj) {
  const file = path.join(tmp('led'), 'specs', 'fleet', 'r16', 'ledger.json');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(obj, null, 2));
  return file;
}
async function run(cases, label, ctx) {
  let failed = 0;
  for (const [name, fn] of cases) {
    try { await fn(ctx); console.log(`PASS ${name}`); } catch (e) { failed++; console.log(`FAIL ${name}\n  ${e && e.message ? e.message.split('\n')[0] : e}`); }
  }
  console.log(failed ? `acceptance 16 (${label}): ${failed} of ${cases.length} failed` : `acceptance 16 (${label}): ${cases.length} passed`);
  return failed;
}
module.exports = { board, get, ledger, ledgerFile, run, TICKETS };
