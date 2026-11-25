// Injected at the end of <body> in a COPY of a page. Drives the page with the mock and writes JSON into #__probe.
// window.__PROBE_MODE: "board" or "popover".
(async () => {
  const out = { errors: [] };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const clean = (s) => (s || '').replace(/\s+/g, ' ').trim();
  try {
    await sleep(600);
    if (window.__PROBE_MODE === 'popover') {
      out.rows = [...document.querySelectorAll('#list button')].map((b) => ({ text: clean(b.innerText), title: b.getAttribute('title') || '' }));
      out.headline = clean(document.getElementById('headline').innerText);
    } else {
      const accent = (() => { const d = document.createElement('div'); d.style.color = 'var(--accent)'; document.body.append(d); const c = getComputedStyle(d).color; d.remove(); return c; })();
      const accentText = (() => { const d = document.createElement('div'); d.style.color = 'var(--accent-text)'; document.body.append(d); const c = getComputedStyle(d).color; d.remove(); return c; })();
      out.accent = accent; out.accentText = accentText;
      const navItems = () => {
        const items = []; let group = '';
        for (const c of document.querySelectorAll('#nav > *')) {
          if (c.classList.contains('thumb')) continue;
          if (c.tagName === 'P' || c.classList.contains('group')) { group = clean(c.textContent); continue; }
          const lbl = c.querySelector('.lbl'), n = c.querySelector('.n'), dot = c.querySelector('.dot');
          items.push({ group, label: clean(lbl ? lbl.textContent : c.textContent), count: n ? clean(n.textContent) : null,
            title: c.getAttribute('title') || '', aria: c.getAttribute('aria-label') || '', text: clean(c.textContent),
            dotBg: dot ? getComputedStyle(dot).backgroundColor : null, dotText: dot ? clean(dot.getAttribute('title') || dot.getAttribute('aria-label') || '') : '' });
        }
        return items;
      };
      const cap = () => {
        const v = document.getElementById('view');
        const rows = [...v.querySelectorAll('li.row')].map((li) => {
          const leaf = (t) => [...li.querySelectorAll('*')].filter((e) => !e.children.length && clean(e.textContent) === t);
          return { title: clean((li.querySelector('.title') || {}).textContent), text: clean(li.innerText),
            mono: ['git push origin main', 'Which database?', 'Add a migration'].flatMap((t) => leaf(t).map((e) => ({ t, ff: getComputedStyle(e).fontFamily }))),
            kinds: [...li.querySelectorAll('*')].filter((e) => !e.children.length && /^(Permission|Question|Plan approval)/.test(clean(e.textContent))).map((e) => ({ t: clean(e.textContent), color: getComputedStyle(e).color })) };
        });
        const lrows = [...v.querySelectorAll('.lrow')].map((r) => ({ title: clean((r.querySelector('.lt') || {}).textContent), text: clean(r.innerText),
          tips: [...r.querySelectorAll('[title],[aria-label]')].map((e) => ({ title: e.getAttribute('title') || '', aria: e.getAttribute('aria-label') || '', svg: !!e.querySelector('svg') })) }));
        const strip = v.querySelector('section.strip');
        const lanes = v.querySelector('.lanes');
        return { rows, lrows, nav: navItems(), counts: [...document.querySelectorAll('#counts button')].map((b) => clean(b.innerText)),
          crumb: clean(document.getElementById('crumb').textContent), strip: strip ? clean(strip.innerText) : '',
          cols: lanes ? getComputedStyle(lanes).gridTemplateColumns : '', titles: rows.map((r) => r.title) };
      };
      const click = (group, label) => {
        const el = [...document.querySelectorAll('#nav > *')]; let g = '';
        for (const c of el) {
          if (c.tagName === 'P' || c.classList.contains('group')) { g = clean(c.textContent); continue; }
          const l = c.querySelector('.lbl');
          if (g === group && l && clean(l.textContent).startsWith(label)) { c.click(); return true; }
        }
        return false;
      };
      out.home = cap();
      out.filters = {};
      const filters = [['Source', 'Pod Bay'], ['Source', 'Terminal'], ['Source', 'Desktop'], ['Projects', 'alpha'], ['Projects', 'beta']];
      for (const [g, l] of filters) {
        const f = { clicked: click(g, l) };
        await sleep(150); f.on = cap();
        click(g, l); await sleep(150); f.off = cap();
        out.filters[`${g}/${l}`] = f;
      }
      // peeks
      const peek = async (title) => {
        const b = document.querySelector(`button[aria-label="Peek at ${title}"]`);
        if (!b) return { missing: true };
        b.click(); await sleep(600);
        const tasks = {};
        for (const t of ['scan model', 'write tests', 'draft plan', 'check docs', 'nested lookup']) {
          const e = [...document.querySelectorAll('#peekBody *')].find((x) => !x.children.length && clean(x.textContent).includes(t));
          const others = ['scan model', 'write tests', 'draft plan', 'check docs', 'nested lookup'].filter((o) => o !== t);
          let row = e;
          while (row && row.parentElement && row.parentElement.id !== 'peekBody' && !others.some((o) => row.parentElement.innerText.includes(o))) row = row.parentElement;
          tasks[t] = e ? { left: e.getBoundingClientRect().left, rowText: clean(row.innerText) } : null;
        }
        const r = { head: clean(document.getElementById('peekMeta').innerText), body: clean(document.getElementById('peekBody').innerText), tasks, openDisabled: document.getElementById('peekOpen').disabled };
        document.getElementById('peekClose').click(); await sleep(200);
        return r;
      };
      out.peek = { 'Fix login': await peek('Fix login'), 'Build local reader': await peek('Build local reader'), 'Pick a database': await peek('Pick a database') };
      // list layout
      document.getElementById('lay-list').click(); await sleep(200);
      out.list = cap();
      document.getElementById('lay-board').click(); await sleep(200);
    }
  } catch (e) { out.errors.push(String((e && e.stack) || e)); }
  const pre = document.createElement('pre'); pre.id = '__probe'; pre.textContent = JSON.stringify(out);
  document.body.append(pre);
})();
