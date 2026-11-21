#!/usr/bin/env node
'use strict';
// The Pod Bay sessions view of the board: the page model (podsOf), and the /pods, /pods.json and /pods/events
// routes against a fake `podbay-dev` (a script that prints fixture JSON). Plain node, no browser.
// Prints one PASS/FAIL line per case and exits 1 on any failure.
// Run: node tests/board/pods.test.js
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');

const root = path.join(__dirname, '..', '..');
const board = require(path.join(root, 'plugins', 'pd-bot', 'board', 'board.js'));
const { podsOf, wsOf, findPodbay, parseArgs, start } = board;

const cases = [];
const test = (name, fn) => cases.push([name, fn]);
const tmp = p => fs.mkdtempSync(path.join(os.tmpdir(), `pods-${p}-`));
const get = (port, p) => new Promise((res, rej) => http.get({ host: '127.0.0.1', port, path: p }, r => {
  let b = ''; r.on('data', c => { b += c; }); r.on('end', () => res({ status: r.statusCode, type: r.headers['content-type'] || '', body: b }));
}).on('error', rej));
// The `data:` events of an SSE stream, until `want` of them arrived.
const events = (port, p, want) => new Promise((res, rej) => {
  const out = []; let buf = '';
  const req = http.get({ host: '127.0.0.1', port, path: p }, r => {
    r.on('data', c => {
      buf += c;
      for (let i; (i = buf.indexOf('\n\n')) >= 0; buf = buf.slice(i + 2)) {
        const line = buf.slice(0, i).split('\n').find(l => l.startsWith('data: '));
        if (line) out.push(JSON.parse(line.slice(6)));
      }
      if (out.length >= want) { req.destroy(); res(out); }
    });
  });
  req.on('error', e => { if (e.code !== 'ECONNRESET') rej(e); });
  setTimeout(() => { req.destroy(); rej(new Error(`only ${out.length} of ${want} events in 5 s`)); }, 5000).unref();
});

const sess = (o) => ({
  title: 'T', name: 'n', uuid: 'UUID-SECRET', project: 'mnemo', repo: 'mnemo', context: 'mnemo/mnemo', dir: '/home/someone/secret/dir',
  status: 'idle', asks: false, waiting_for: null, since: 1000, changes: [0, 0], description: null,
  links: { prs: [], issues: [], artifacts: [] }, attached: false, account: 'someone@example.com', plan: 'max', kind: 'claude',
  remote_url: 'git@example.com:owner/secret.git', ...o,
});
const RAW = {
  ls: [
    { machine: 'UA', result: [
      sess({ title: 'Idle old', context: 'mnemo/mnemo@02', since: 1000, dir: '/d/02' }),
      sess({ title: 'Idle new', context: 'mnemo/mnemo@03', since: 9000 }),
      sess({ title: 'Busy', context: 'pb-bot/pb-bot@06', project: 'pb-bot', repo: 'pb-bot', status: 'busy', since: 5000, description: 'board work' }),
      sess({ title: 'Asks late', context: 'mnemo/mnemo@01', asks: true, since: 4000, links: { prs: [{ url: 'u1' }, { url: 'u2' }] } }),
      sess({ title: 'Asks early', context: 'mnemo/mnemo', asks: true, status: 'busy', since: 2000 }),
    ] },
    { machine: 'M0', result: [] },
  ],
  checkouts: [
    { machine: 'UA', result: [
      { project: 'mnemo', repo: 'mnemo', ws: 'src', branch: 'main', dirty: 0, has_conversation: true, path: '/home/someone/secret/path' },
      { project: 'mnemo', repo: 'mnemo', ws: 'src-01', branch: 'feat/x', dirty: 0, has_conversation: true },
      { project: 'mnemo', repo: 'mnemo', ws: 'src-07', branch: 'main', dirty: 0, has_conversation: true },
      { project: 'mnemo', repo: 'extra', ws: 'src-07', branch: null, dirty: 1, has_conversation: false },
    ] },
    { machine: 'U0', result: [{ project: 'harness', repo: 'agent-harness', ws: 'src', branch: 'main', dirty: 0, has_conversation: true }] },
  ],
  machines: [{ machine: 'UA', online: true, id: 'ID-SECRET', addr: '10.0.0.1:7880' }, { machine: 'Old', online: false }],
  queue: [{ machine: 'UA', result: [{ dir: '/d/02', finished: null }, { dir: '/d/02', finished: 123 },
    { dir: '/d/02', state: 'unknown', finished: null }, { dir: '/d/02', state: 'delivered', finished: 5 }] }],
};

test('wsOf: src for a plain context, src-NN for @NN', () => {
  assert.strictEqual(wsOf('mnemo/mnemo'), 'src');
  assert.strictEqual(wsOf('mnemo/mnemo@05'), 'src-05');
  assert.strictEqual(wsOf(undefined), 'src');
});

test('podsOf: asks wins over status; order is needs, busy, idle', () => {
  const m = podsOf(RAW);
  assert.deepStrictEqual(m.sessions.map(s => [s.title, s.state]), [
    ['Asks early', 'needs'], ['Asks late', 'needs'], ['Busy', 'busy'], ['Idle new', 'idle'], ['Idle old', 'idle'],
  ]);
  assert.deepStrictEqual(m.counts, { needs: 2, busy: 1, idle: 2, stopped: 2 });
});

test('podsOf: a session with no status yet (stopped before its prompt) needs a human', () => {
  const m = podsOf({ ls: [{ machine: 'M0', result: [sess({ title: 'Starting', status: null }), sess({ title: 'Ready' })] }] });
  assert.deepStrictEqual(m.sessions.map(s => [s.title, s.state]), [['Starting', 'needs'], ['Ready', 'idle']]);
});

test('podsOf: workspace, description, branch fallback, PR count and queue count', () => {
  const by = Object.fromEntries(podsOf(RAW).sessions.map(s => [s.title, s]));
  assert.strictEqual(by['Asks late'].ws, 'src-01');
  assert.strictEqual(by['Asks late'].branch, 'feat/x');
  assert.strictEqual(by['Asks late'].what, '');
  assert.strictEqual(by['Asks late'].prs, 2);
  assert.strictEqual(by.Busy.what, 'board work');
  assert.strictEqual(by['Idle old'].queued, 1, 'one unfinished queue entry for its dir');
  assert.strictEqual(by['Idle new'].queued, 0);
});

test('podsOf: a workspace with no session is stopped, once per workspace', () => {
  const m = podsOf(RAW);
  assert.deepStrictEqual(m.stopped, [
    { machine: 'U0', project: 'harness', ws: 'src', dirty: false },
    { machine: 'UA', project: 'mnemo', ws: 'src-07', dirty: true },
  ]);
  assert.deepStrictEqual(m.machines, [{ name: 'UA', online: true }, { name: 'Old', online: false }]);
});

test('podsOf: account, uuid, dir, path, remote_url and machine ids never reach the model', () => {
  const body = JSON.stringify(podsOf(RAW));
  for (const secret of ['someone@example.com', 'UUID-SECRET', '/home/someone', 'secret.git', 'ID-SECRET', '10.0.0.1']) {
    assert.ok(!body.includes(secret), `model holds ${secret}`);
  }
});

test('podsOf: missing or malformed input gives an empty model, not a throw', () => {
  for (const raw of [undefined, {}, { ls: 'x', checkouts: [{ machine: 'A', result: 'err' }], machines: {}, queue: null }]) {
    assert.deepStrictEqual(podsOf(raw), { machines: [], sessions: [], stopped: [], counts: { needs: 0, busy: 0, idle: 0, stopped: 0 } });
  }
});

test('parseArgs: --home and --podbay; a bad --home is refused', () => {
  const a = parseArgs(['--home', 'pods', '--podbay', '/x/fake']);
  assert.strictEqual(a.home, 'pods'); assert.strictEqual(a.podbay, '/x/fake');
  assert.strictEqual(parseArgs([]).home, 'run');
  assert.throws(() => parseArgs(['--home', 'nope']), /--home/);
});

test('findPodbay: PATH first, then the install folders, else the bare name', () => {
  const home = path.join(os.homedir(), '.local', 'bin', 'podbay-dev');
  assert.strictEqual(findPodbay({ PATH: ['/a', '/b'].join(path.delimiter) }, f => f === path.join('/b', 'podbay-dev')), path.join('/b', 'podbay-dev'));
  assert.strictEqual(findPodbay({ PATH: '/a' }, f => f === home), home);
  assert.strictEqual(findPodbay({}, () => false), 'podbay-dev');
});

test('parseArgs: $PORT gives the default port; an out-of-range $PORT is ignored', () => {
    const old = process.env.PORT;
    try {
      process.env.PORT = '5123'; assert.strictEqual(parseArgs([]).port, 5123);
      assert.strictEqual(parseArgs(['--port', '6000']).port, 6000);
      process.env.PORT = '99999'; assert.strictEqual(parseArgs([]).port, 4317);
    } finally { if (old === undefined) delete process.env.PORT; else process.env.PORT = old; }
});

// A fake podbay-dev: prints the fixture for its subcommand; exits 1 when the file `fail` is next to it.
function fake() {
  const dir = tmp('fake');
  for (const k of ['ls', 'checkouts', 'machines', 'queue']) fs.writeFileSync(path.join(dir, `${k}.json`), JSON.stringify(RAW[k]));
  const file = path.join(dir, 'podbay-fake');
  fs.writeFileSync(file, '#!/usr/bin/env bash\nd=$(dirname "$0")\n[ -e "$d/fail" ] && { echo "boom /home/someone/secret" >&2; exit 1; }\ncat "$d/$1.json"\n', { mode: 0o755 });
  return { dir, file };
}
async function withBoard(opts, fn) {
  const b = await start({ port: 0, project: tmp('proj'), projectDir: tmp('pd'), podsMs: 60, ...opts });
  try { return await fn(b.port); } finally { await new Promise(r => b.server.close(r)); }
}

test('/pods serves the page; --home pods serves it at / too; the run page stays at / by default', async () => {
  const f = fake();
  await withBoard({ podbay: f.file }, async port => {
    const p = await get(port, '/pods');
    assert.strictEqual(p.status, 200); assert.match(p.type, /text\/html/); assert.match(p.body, /Pod board/);
    assert.doesNotMatch((await get(port, '/')).body, /<title>Pod board/);
    assert.strictEqual((await get(port, '/healthz')).status, 200);
  });
  await withBoard({ podbay: f.file, home: 'pods' }, async port => {
    assert.match((await get(port, '/')).body, /<title>Pod board/);
  });
});

test('/pods.json reads Pod Bay once and returns the model', async () => {
  const f = fake();
  await withBoard({ podbay: f.file }, async port => {
    const r = await get(port, '/pods.json');
    assert.strictEqual(r.status, 200); assert.match(r.type, /application\/json/);
    const j = JSON.parse(r.body);
    assert.strictEqual(j.ok, true); assert.strictEqual(j.error, null); assert.ok(j.read_at > 0);
    assert.deepStrictEqual(j.counts, { needs: 2, busy: 1, idle: 2, stopped: 2 });
    assert.ok(!r.body.includes('someone@example.com'));
  });
});

test('/pods.json: a missing command is reported, the board keeps running', async () => {
  await withBoard({ podbay: path.join(tmp('none'), 'no-such-podbay') }, async port => {
    const j = JSON.parse((await get(port, '/pods.json')).body);
    assert.strictEqual(j.ok, false); assert.match(j.error, /no-such-podbay not found/);
    assert.deepStrictEqual(j.sessions, []);
    assert.strictEqual((await get(port, '/healthz')).status, 200);
  });
});

test('/pods/events: a snapshot first, then ticks; a failing read keeps the last sessions and hides stderr', async () => {
  const f = fake();
  await withBoard({ podbay: f.file }, async port => {
    const ev = await events(port, '/pods/events', 4);
    assert.strictEqual(ev[0].kind, 'snapshot', 'a stream always starts with a snapshot');
    const full = ev.find(e => e.kind === 'snapshot' && e.ok);
    assert.ok(full, 'no read snapshot arrived'); assert.strictEqual(full.sessions.length, 5);
    assert.strictEqual(ev[ev.length - 1].kind, 'tick'); assert.strictEqual(ev[ev.length - 1].ok, true);
    fs.writeFileSync(path.join(f.dir, 'fail'), '');
    const bad = (await events(port, '/pods/events', 3)).find(e => e.kind === 'snapshot' && !e.ok);
    assert.ok(bad, 'no failed snapshot arrived');
    assert.match(bad.error, /podbay-fake ls failed/);
    assert.ok(!JSON.stringify(bad).includes('boom'), 'stderr reached the page');
    assert.strictEqual(bad.sessions.length, 5, 'the last known sessions stay');
  });
});

test('/pods/events: a page that connects while every read fails still gets the error', async () => {
  await withBoard({ podbay: path.join(tmp('none'), 'no-such-podbay') }, async port => {
    const one = await events(port, '/pods/events', 3);
    assert.ok(one.some(e => e.kind === 'snapshot' && e.error), 'the first page got no error snapshot');
    const two = await events(port, '/pods/events', 2);
    assert.strictEqual(two[0].kind, 'snapshot');
    assert.ok(two.some(e => e.kind === 'snapshot' && /not found/.test(e.error || '')), 'the second page got no error snapshot');
  });
});

(async () => {
  let failed = 0;
  for (const [name, fn] of cases) {
    try { await fn(); console.log(`PASS ${name}`); } catch (e) { failed++; console.log(`FAIL ${name}\n  ${e.stack || e.message}`); }
  }
  console.log(`pods: ${cases.length - failed}/${cases.length} passed`);
  process.exit(failed ? 1 : 0);
})();
