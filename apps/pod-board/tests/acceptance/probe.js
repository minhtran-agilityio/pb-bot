// Injected at the end of <body> in a COPY of a page (never in the app). Runs after load, writes JSON into #__probe.
// Mode comes from window.__PROBE_MODE: "board" or "popover".
(async () => {
  const out = { errors: [] };
  const rgb = (c) => c;
  try {
    const root = document.documentElement;
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    await sleep(300);
    // fonts
    const loadFont = async (spec) => { try { const f = await document.fonts.load(spec); return f.length > 0; } catch (e) { return false; } };
    out.fonts = {
      outfit400: await loadFont('400 14px Outfit'),
      outfit500: await loadFont('500 14px Outfit'),
      outfit600: await loadFont('600 14px Outfit'),
      jbmono400: await loadFont('400 12px "JetBrains Mono"'),
    };
    out.bodyFont = getComputedStyle(document.body).fontFamily;
    // banned computed styles on every rendered element
    const bad = [];
    for (const el of document.querySelectorAll('body *')) {
      if (el.closest('svg')) continue;
      const cs = getComputedStyle(el);
      const ownText = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
      if (cs.display === 'none') continue;
      if (cs.textTransform === 'uppercase') bad.push(['uppercase', el.tagName, el.id || el.className]);
      if (cs.backdropFilter && cs.backdropFilter !== 'none') bad.push(['backdrop-filter', el.tagName, el.id || el.className]);
      if (ownText && parseInt(cs.fontWeight, 10) > 600) bad.push(['weight>600', el.tagName, el.id || el.className, cs.fontWeight]);
    }
    out.bad = bad;
    // self-check that :focus-visible matches for scripted focus in this browser
    const sanity = document.createElement('button');
    sanity.textContent = 'x';
    const st = document.createElement('style');
    st.textContent = '#__sanity:focus-visible{outline:3px solid rgb(1,2,3)}';
    sanity.id = '__sanity'; document.body.append(st, sanity); sanity.focus();
    out.focusVisibleWorks = getComputedStyle(sanity).outlineColor === 'rgb(1, 2, 3)';
    sanity.remove(); st.remove();

    const snap = (theme) => {
      root.dataset.theme = theme;
      const cs = getComputedStyle(root);
      const tok = (n) => cs.getPropertyValue(n).trim();
      const bgOf = (el) => (el ? getComputedStyle(el).backgroundColor : null);
      const focus = [];
      for (const b of document.querySelectorAll('button, a[href], [tabindex]:not([tabindex="-1"])')) {
        const r = b.getBoundingClientRect();
        if (!r.width || !r.height || getComputedStyle(b).visibility === 'hidden') continue;
        b.focus();
        const c = getComputedStyle(b);
        focus.push({ id: b.id || b.className || b.tagName, style: c.outlineStyle, width: c.outlineWidth, offset: c.outlineOffset, color: c.outlineColor });
        b.blur();
      }
      const ringProbe = document.createElement('div'); ringProbe.style.color = 'var(--ring)'; document.body.append(ringProbe);
      const ringRgb = getComputedStyle(ringProbe).color; ringProbe.remove();
      return {
        body: bgOf(document.body), html: bgOf(root), main: bgOf(document.querySelector('main')), side: bgOf(document.getElementById('side')),
        bgTok: tok('--bg'), ringRgb, focus,
      };
    };
    out.light = snap('light');
    out.dark = snap('dark');
    root.dataset.theme = 'light';

    if (window.__PROBE_MODE === 'board') {
      const rect = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; };
      const side = document.getElementById('side');
      const main = document.querySelector('main');
      const shell = document.querySelector('.shell');
      out.layout = {
        side: rect(side), main: rect(main),
        shellGap: shell ? getComputedStyle(shell).columnGap : null,
        sideRadius: side ? getComputedStyle(side).borderTopLeftRadius : null,
        mainRadius: main ? getComputedStyle(main).borderTopLeftRadius : null,
      };
      out.titlebars = [...document.querySelectorAll('[data-tauri-drag-region]')].map((el) => ({ text: el.textContent.replace(/\s+/g, ' ').trim(), rect: rect(el), hasGraphic: !!el.querySelector('svg, img') }));
      out.brandText = document.body.innerText.includes('Session control room');
      out.themeButtonsInSide = ['th-light', 'th-dark', 'th-system'].map((i) => { const e = document.getElementById(i); return !!(e && side && side.contains(e)); });
      out.cats = [...document.querySelectorAll('#cat, .cat')].map((e) => getComputedStyle(e).position);
      // theme switch effect
      const dark = document.getElementById('th-dark'); if (dark) dark.click();
      await sleep(100);
      out.afterDarkClick = root.dataset.theme;
      const light = document.getElementById('th-light'); if (light) light.click();
      await sleep(100);
      out.afterLightClick = root.dataset.theme;
      // collapse
      const btn = document.getElementById('sidebar');
      if (btn) { btn.click(); await sleep(400); }
      out.collapsed = {
        expanded: btn ? btn.getAttribute('aria-expanded') : null,
        sideW: rect(side) && rect(side).w,
        tips: [...document.querySelectorAll('#nav button, #nav a')].map((e) => ({ label: e.textContent.trim(), title: e.getAttribute('title') || e.getAttribute('aria-label') || '' })),
      };
      if (btn) { btn.click(); await sleep(400); }
      out.reexpanded = { expanded: btn ? btn.getAttribute('aria-expanded') : null, sideW: rect(side) && rect(side).w };
    }
  } catch (e) { out.errors.push(String(e && e.stack || e)); }
  const pre = document.createElement('pre'); pre.id = '__probe'; pre.textContent = JSON.stringify(out);
  document.body.append(pre);
})();
