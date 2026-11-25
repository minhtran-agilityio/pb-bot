// Reads the resolved value of each named custom property. Names come from window.__NAMES.
(() => {
  const out = {};
  for (const theme of ['light', 'dark']) {
    document.documentElement.dataset.theme = theme;
    const cs = getComputedStyle(document.documentElement);
    out[theme] = {};
    for (const n of window.__NAMES) out[theme][n] = cs.getPropertyValue(n).trim();
  }
  const pre = document.createElement('pre'); pre.id = '__probe'; pre.textContent = JSON.stringify(out);
  document.body.append(pre);
})();
