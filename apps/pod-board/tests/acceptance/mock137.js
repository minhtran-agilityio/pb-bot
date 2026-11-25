// Mock of the Tauri runtime for the #137 page tests. Injected at the start of <head> in a COPY of a page.
// It answers `snapshot` with the data below and `peek` with a fixed prompt and reply. See README.md "Mock contract".
(() => {
  const now = Date.now(), MIN = 60000;
  const desktop = !!window.__WITH_DESKTOP;
  const s = (o) => Object.assign({ ws: 'src', machine: 'M0', waiting: '', what: '', branch: '', links: [], prs: 0, queued: 0 }, o);
  const sessions = [
    s({ title: 'Fix login', name: 'fix-login', rc_id: 'rc1', state: 'needs', since: now - 5 * MIN, project: 'pb-bot', source: 'podbay',
        waiting: 'input needed', what: 'POD BAY DESC A', reason: { kind: 'permission', detail: 'git push origin main' } }),
    s({ title: 'Build local reader', name: 'build-local', rc_id: 'rc2', state: 'busy', since: now - 9 * MIN, project: 'pb-bot', ws: 'src2', source: 'podbay',
        what: 'POD BAY DESC B', step: 'Edit: src/local.rs',
        agents: { running: 4, done: 1, list: [
          { type: 'Explore', task: 'scan model', model: 'haiku', since: now - 4 * MIN, done: false, depth: 0, step: 'Read: model.rs' },
          { type: 'general-purpose', task: 'write tests', model: 'sonnet', since: now - 3 * MIN, done: false, depth: 0, step: 'Bash: cargo test' },
          { type: 'Plan', task: 'draft plan', model: 'opus', since: now - 2 * MIN, done: false, depth: 0, step: 'Write: plan' },
          { type: 'Explore', task: 'check docs', model: 'haiku', since: now - 1 * MIN, done: false, depth: 0, step: 'Grep: README' },
          { type: 'Explore', task: 'nested lookup', model: 'haiku', since: now - 6 * MIN, done: true, took_s: 95, result: '3 files', depth: 1 },
        ] } }),
    s({ title: 'Review notes', name: 'review-notes', rc_id: 'rc3', state: 'needs', since: now - 3 * MIN, project: 'gamma', machine: 'U0', source: 'podbay',
        waiting: 'input needed', what: 'POD BAY DESC E' }),
    s({ title: 'Remote worker', name: 'remote-worker', rc_id: 'rc4', state: 'busy', since: now - 7 * MIN, project: 'gamma', machine: 'U0', source: 'podbay',
        what: 'POD BAY DESC G', agents: { running: 0, done: 2, list: [
          { type: 'Explore', task: 'remote one', model: 'haiku', since: now - 8 * MIN, done: true, took_s: 40, result: 'ok', depth: 0 },
          { type: 'Explore', task: 'remote two', model: 'haiku', since: now - 8 * MIN, done: true, took_s: 50, result: 'ok', depth: 0 } ] } }),
    s({ title: 'Pick a database', name: 'pick-db', state: 'needs', since: now - 2 * MIN, project: 'alpha', source: 'terminal',
        waiting: 'input needed', reason: { kind: 'question', detail: 'Which database?' } }),
    s({ title: 'Terminal tidy', name: 'tidy', state: 'idle', since: now - 60 * MIN, project: 'beta', source: 'terminal', what: 'DESC D' }),
    s({ title: 'Notes', name: 'notes', state: 'idle', since: now - 90 * MIN, project: 'beta', source: 'terminal', what: 'DESC H' }),
  ];
  const stopped = [{ project: 'pb-bot', ws: 'old', machine: 'M0', dirty: false }];
  if (desktop) {
    sessions.push(
      s({ title: 'Index repo', name: 'index-repo', state: 'busy', since: now - 4 * MIN, project: 'alpha', source: 'desktop', what: 'DESC C' }),
      s({ title: 'Ask about schema', name: 'ask-schema', state: 'needs', since: now - 1 * MIN, project: 'alpha', source: 'desktop',
          waiting: 'input needed', reason: { kind: 'plan', detail: 'Add a migration' } }));
    stopped.push({ source: 'desktop', title: 'Closed chat', project: 'alpha', ws: '', machine: 'M0', closed: now - 3600000 });
  }
  const counts = { needs: 0, busy: 0, idle: 0, stopped: stopped.length };
  for (const x of sessions) counts[x.state]++;
  const snap = { ok: true, error: null, read_at: now, dnd_until: null, counts, sessions, stopped,
    machines: [{ name: 'M0', online: true }, { name: 'U0', online: true }, { name: 'U1', online: false, key_stale: true }] };
  window.__MOCK_SNAP = snap;
  window.__TAURI__ = {
    core: { invoke: (cmd, args) => {
      if (cmd === 'snapshot') return Promise.resolve(JSON.parse(JSON.stringify(snap)));
      if (cmd === 'peek') return Promise.resolve({ shared: true, prompt: 'Please push the fix', reply: 'I need permission to push.', at: now - MIN });
      return Promise.resolve(null);
    } },
    event: { listen: () => Promise.resolve(() => {}) },
  };
})();
