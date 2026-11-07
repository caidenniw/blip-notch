(function () {
  'use strict';
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const $ = s => document.querySelector(s);
  const $$ = s => Array.from(document.querySelectorAll(s));

  let conf = null;    // blip.config.json (c.* keys)
  let prefs = null;   // island localStorage prefs (p.* keys)
  let autostart = false;

  const PAGES = ['today', 'music', 'focus', 'todo', 'water', 'shelf'];
  // [min, max] per numeric pref; mirrored by the island's pref-set validation.
  const LIMITS = {
    'p.focusDur.focus': [1, 120], 'p.focusDur.short': [1, 120], 'p.focusDur.long': [1, 120],
    'p.water.goal': [500, 6000], 'p.water.cup': [50, 1000], 'p.water.every': [15, 240],
    'p.water.start': [0, 23], 'p.water.end': [1, 24],
  };

  let statusTimer = 0;
  function status(msg, err) {
    const el = $('#status');
    el.textContent = String(msg || '');
    el.classList.toggle('err', !!err);
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => { el.textContent = ''; el.classList.remove('err'); }, 2500);
  }

  function path(obj, keys) {
    return keys.reduce((o, k) => (o == null ? undefined : o[k]), obj);
  }

  /// Current value of a data-k key.
  function read(k) {
    if (k === 'autostart') return autostart;
    if (k.startsWith('c.')) {
      if (!conf) return undefined;
      const v = path(conf, k.slice(2).split('.'));
      return k === 'c.router.models' ? (v || []).join(', ') : v;
    }
    if (!prefs) return undefined;
    if (k.startsWith('p.page.')) return !prefs.pagesOff.includes(k.slice(7));
    return path(prefs, k.slice(2).split('.'));
  }

  /// `force` also rewrites the focused input (revert after a rejected save).
  function render(force) {
    for (const el of $$('[data-k]')) {
      const k = el.dataset.k;
      const v = read(k);
      const off = v === undefined;
      if (off) el.setAttribute('data-off', ''); else el.removeAttribute('data-off');
      if (el.classList.contains('sw')) el.classList.toggle('on', !!v);
      else if (el.classList.contains('seg')) {
        for (const b of el.querySelectorAll('button')) b.classList.toggle('on', !off && b.dataset.v === String(v));
      } else if (force || document.activeElement !== el) el.value = off ? '' : String(v);
    }
  }

  async function commitConf(k, raw) {
    const next = JSON.parse(JSON.stringify(conf));
    const keys = k.slice(2).split('.');
    let v = raw;
    if (k === 'c.router.models') v = String(raw).split(',').map(s => s.trim()).filter(Boolean);
    else if (k === 'c.ui.autoCollapseMs') v = Number(raw);
    path(next, keys.slice(0, -1))[keys[keys.length - 1]] = v;
    try {
      await invoke('config_set', { conf: next });
      conf = next;
      status('Tersimpan');
      render();
    } catch (err) {
      status(err, true);
      render(true);
    }
  }

  function commitPref(k, raw) {
    let key, value;
    if (k.startsWith('p.page.')) {
      const id = k.slice(7);
      const off = prefs.pagesOff.filter(p => p !== id);
      if (!raw) off.push(id);
      key = 'pagesOff'; value = off;
    } else if (k.startsWith('p.focusDur.') || k.startsWith('p.water.')) {
      const [group, field] = k.slice(2).split('.');
      const [lo, hi] = LIMITS[k];
      const n = Math.round(Number(raw));
      if (!Number.isFinite(n)) { status('Angka tidak valid', true); render(true); return; }
      value = Object.assign({}, prefs[group], { [field]: Math.max(lo, Math.min(hi, n)) });
      if (group === 'water' && value.end <= value.start) {
        status('Jam selesai harus setelah jam mulai', true);
        render(true);
        return;
      }
      key = group;
    } else {
      key = k.slice(2);
      value = key === 'closeDelay' ? Number(raw) : raw;
    }
    invoke('pref_set', { key, value }).catch(() => {});
    prefs[key] = value;
    status('Tersimpan');
    render();
  }

  function commit(k, v) {
    if (read(k) === undefined) return;
    if (k === 'autostart') {
      invoke('autostart_set', { on: !!v })
        .catch(err => status(err, true))
        .then(() => invoke('autostart_get'))
        .then(on => { autostart = !!on; render(); });
    } else if (k.startsWith('c.')) commitConf(k, v);
    else commitPref(k, v);
  }

  document.addEventListener('click', e => {
    const nav = e.target.closest('.navb');
    if (nav) { document.body.dataset.sec = nav.dataset.s; return; }
    const sw = e.target.closest('.sw[data-k]');
    if (sw) { commit(sw.dataset.k, !sw.classList.contains('on')); return; }
    const b = e.target.closest('.seg[data-k] button');
    if (b) { commit(b.closest('.seg').dataset.k, b.dataset.v); return; }
    const chip = e.target.closest('.chip');
    if (chip && conf) {
      const id = chip.dataset.id;
      commitConf('c.router.models', [id].concat(conf.router.models.filter(m => m !== id)).join(','));
    }
  });
  for (const el of $$('input[data-k]')) {
    el.addEventListener('change', () => commit(el.dataset.k, el.value));
  }

  const testBtn = $('#test');
  testBtn.addEventListener('click', async () => {
    if (!conf || testBtn.hasAttribute('data-busy')) return;
    testBtn.setAttribute('data-busy', '');
    testBtn.textContent = 'Menguji…';
    try {
      const ids = await invoke('router_test', { baseUrl: conf.router.baseUrl, apiKey: conf.router.apiKey });
      status('Terhubung · ' + ids.length + ' model');
      const box = $('#models');
      box.innerHTML = '';
      for (const id of ids.slice(0, 12)) {
        const c = document.createElement('button');
        c.className = 'chip';
        c.dataset.id = id;
        c.textContent = id;
        box.appendChild(c);
      }
    } catch (err) {
      status(err, true);
    }
    testBtn.removeAttribute('data-busy');
    testBtn.textContent = 'Tes koneksi';
  });

  $('#cfgFile').addEventListener('click', () => invoke('open_config_file').catch(err => status(err, true)));

  listen('prefs', e => {
    const p = e.payload || {};
    p.pagesOff = Array.isArray(p.pagesOff) ? p.pagesOff.filter(x => PAGES.includes(x)) : [];
    prefs = p;
    render();
  });

  render();
  invoke('config_get').then(c => { conf = c; render(); }).catch(err => status(err, true));
  invoke('autostart_get').then(on => { autostart = !!on; render(); }).catch(() => {});
  invoke('prefs_request').catch(() => {});
})();
