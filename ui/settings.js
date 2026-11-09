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
  const DEFAULT_BASE = 'https://api.openai.com/v1';
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
    if (k.startsWith('c.')) return conf ? path(conf, k.slice(2).split('.')) : undefined;
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
    if (conf) {
      document.body.dataset.stt = conf.stt.provider;
      document.body.dataset.tts = conf.tts.provider;
    }
    renderAi();
  }

  async function commitConf(k, raw) {
    const next = JSON.parse(JSON.stringify(conf));
    const keys = k.slice(2).split('.');
    const v = k === 'c.ui.autoCollapseMs' ? Number(raw) : raw;
    path(next, keys.slice(0, -1))[keys[keys.length - 1]] = v;
    try {
      await invoke('config_set', { conf: next });
      const langChanged = next.ui.language !== conf.ui.language;
      conf = next;
      if (langChanged) {
        setLang(next.ui.language);
        applyI18n();
        render(true);
      } else render();
      status(t('set.saved'));
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
      if (!Number.isFinite(n)) { status(t('set.badNumber'), true); render(true); return; }
      value = Object.assign({}, prefs[group], { [field]: Math.max(lo, Math.min(hi, n)) });
      if (group === 'water' && value.end <= value.start) {
        status(t('set.badHours'), true);
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
    status(t('set.saved'));
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

  // ---------- AI: draft + Connect (saved only after the main model answers) ----------
  const ICON = {
    idle: '<circle cx="12" cy="12" r="4" fill="currentColor"/>',
    testing: '<circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="2" stroke-dasharray="4 3"/>',
    ok: '<circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"/><path d="m7.5 12.5 3 3 6-6.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>',
    fail: '<circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"/><path d="m9 9 6 6M15 9l-6 6" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>',
  };
  const EYE = '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="12" cy="12" r="3" fill="none" stroke="currentColor" stroke-width="1.8"/>';
  const EYE_OFF = EYE + '<path d="M4 4l16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>';

  const aiBase = $('#aiBase');
  const aiKey = $('#aiKey');
  const aiModel = $('#aiModel');
  const bkNew = $('#bkNew');
  const connEl = $('#conn');
  let draft = null;      // { baseUrl, apiKey, model, backups[] }
  let card = { st: 'idle' }; // last Connect result shown on the card
  let busy = false;
  let dots = {};         // backup id -> { ok, title }

  function draftFromConf() {
    const m = conf.router.models || [];
    draft = { baseUrl: conf.router.baseUrl, apiKey: conf.router.apiKey, model: m[0] || '', backups: m.slice(1) };
    aiBase.value = draft.baseUrl;
    aiKey.value = draft.apiKey;
    aiModel.value = draft.model;
  }
  function draftModels() { return [draft.model.trim()].concat(draft.backups).filter(Boolean); }
  function dirty() {
    if (!conf || !draft) return false;
    return draft.baseUrl.trim() !== conf.router.baseUrl || draft.apiKey.trim() !== conf.router.apiKey
      || draftModels().join('\n') !== (conf.router.models || []).join('\n');
  }
  function host(url) {
    const m = /^https?:\/\/([^/:?#]+)/i.exec(url.trim());
    return m ? m[1] : url;
  }

  function renderBackups() {
    const box = $('#bkList');
    box.textContent = '';
    draft.backups.forEach((id, i) => {
      const row = document.createElement('div');
      row.className = 'bk';
      const code = document.createElement('code');
      code.className = 'mid';
      code.textContent = id;
      const dot = document.createElement('i');
      const d = dots[id];
      dot.className = 'dot' + (d ? (d.ok ? ' ok' : ' bad') : '');
      if (d && d.title) dot.setAttribute('title', d.title);
      const x = document.createElement('button');
      x.className = 'x';
      x.dataset.i = String(i);
      x.textContent = '×';
      x.setAttribute('title', t('ai.remove'));
      row.append(code, dot, x);
      box.appendChild(row);
    });
  }

  function renderAi() {
    if (!conf) return;
    if (!draft) draftFromConf();
    renderBackups();
    const st = busy ? 'testing' : card.st;
    connEl.dataset.st = st;
    connEl.querySelector('.ic svg').innerHTML = ICON[st];
    $('#connModel').textContent = draft.model.trim() || conf.router.models[0] || '';
    let sub = '';
    let detail = '';
    if (st === 'testing') sub = t('ai.testing');
    else if (st === 'ok') sub = t('ai.ok', { ms: card.ms, host: card.host });
    else if (st === 'fail') { sub = card.key ? t(card.key) : ''; detail = card.detail || ''; }
    else sub = t(dirty() ? 'ai.unsaved' : 'ai.notTested');
    $('#connSub').textContent = sub;
    $('#connDetail').textContent = detail;
    const discard = $('#discard');
    if (dirty() && !busy) discard.removeAttribute('hidden'); else discard.setAttribute('hidden', '');
    for (const el of [aiBase, aiKey, aiModel, bkNew, $('#bkAdd'), $('#connect'), discard]) {
      if (busy) el.setAttribute('disabled', ''); else el.removeAttribute('disabled');
    }
    $('#aiEye svg').innerHTML = aiKey.getAttribute('type') === 'password' ? EYE : EYE_OFF;
  }

  function onDraftInput() {
    if (!draft || busy) return;
    draft.baseUrl = aiBase.value;
    draft.apiKey = aiKey.value;
    draft.model = aiModel.value;
    aiBase.classList.remove('bad');
    aiModel.classList.remove('bad');
    if (card.st !== 'idle') card = { st: 'idle' };
    renderAi();
  }
  for (const el of [aiBase, aiKey, aiModel]) el.addEventListener('input', onDraftInput);

  function addBackup() {
    if (!draft || busy) return;
    const id = bkNew.value.trim();
    if (!id || id === draft.model.trim() || draft.backups.includes(id)) { bkNew.value = ''; return; }
    draft.backups.push(id);
    bkNew.value = '';
    card = { st: 'idle' };
    renderAi();
  }
  $('#bkAdd').addEventListener('click', addBackup);
  bkNew.addEventListener('keydown', e => { if (e.key === 'Enter') addBackup(); });

  $('#aiEye').addEventListener('click', () => {
    aiKey.setAttribute('type', aiKey.getAttribute('type') === 'password' ? 'text' : 'password');
    aiKey.value = aiKey.value; // re-shape the masked text
    renderAi();
  });

  $('#discard').addEventListener('click', () => {
    if (busy || !conf) return;
    draftFromConf();
    dots = {};
    card = { st: 'idle' };
    aiBase.classList.remove('bad');
    aiModel.classList.remove('bad');
    renderAi();
  });

  function fail(key, detail) {
    card = { st: 'fail', key, detail };
    renderAi();
  }

  $('#connect').addEventListener('click', async () => {
    if (!conf || !draft || busy) return;
    onDraftInput();
    const baseUrl = draft.baseUrl.trim();
    const apiKey = draft.apiKey.trim();
    const model = draft.model.trim();
    if (!/^https?:\/\/\S+$/.test(baseUrl)) { aiBase.classList.add('bad'); return fail('ai.err.endpoint'); }
    if (!model) { aiModel.classList.add('bad'); return fail('ai.err.model'); }
    const models = [model].concat(draft.backups.filter(m => m !== model));
    busy = true;
    renderAi();
    try {
      const res = await invoke('ai_connect', { baseUrl, apiKey, models });
      dots = {};
      for (const r of res.slice(1)) dots[r.model] = { ok: r.ok, title: r.ok ? r.ms + ' ms' : t('ai.err.' + r.code) + (r.detail ? ' · ' + r.detail : '') };
      const main = res[0];
      if (!main.ok) {
        busy = false;
        return fail('ai.err.' + main.code, main.detail);
      }
      const next = JSON.parse(JSON.stringify(conf));
      next.router = Object.assign({}, conf.router, { baseUrl, apiKey, models });
      try {
        await invoke('config_set', { conf: next });
        conf = next;
        draftFromConf();
        card = { st: 'ok', ms: main.ms, host: host(baseUrl) };
        status(t('set.saved'));
      } catch (err) {
        card = { st: 'fail', key: '', detail: String(err) };
      }
    } catch (err) {
      card = { st: 'fail', key: 'ai.err.http', detail: String(err) };
    }
    busy = false;
    renderAi();
  });

  document.addEventListener('click', e => {
    const nav = e.target.closest('.navb');
    if (nav) { document.body.dataset.sec = nav.dataset.s; return; }
    const x = e.target.closest('.bk .x');
    if (x && draft && !busy) {
      draft.backups.splice(Number(x.dataset.i), 1);
      card = { st: 'idle' };
      renderAi();
      return;
    }
    const sw = e.target.closest('.sw[data-k]');
    if (sw) { commit(sw.dataset.k, !sw.classList.contains('on')); return; }
    const b = e.target.closest('.seg[data-k] button');
    if (b) commit(b.closest('.seg').dataset.k, b.dataset.v);
  });
  for (const el of $$('input[data-k]')) {
    el.addEventListener('change', () => commit(el.dataset.k, el.value));
  }

  $('#cfgFile').addEventListener('click', () => invoke('open_config_file').catch(err => status(err, true)));

  listen('prefs', e => {
    const p = e.payload || {};
    p.pagesOff = Array.isArray(p.pagesOff) ? p.pagesOff.filter(x => PAGES.includes(x)) : [];
    prefs = p;
    render();
  });

  applyI18n();
  render();
  invoke('config_get').then(c => {
    conf = c;
    if (!c.router.apiKey && c.router.baseUrl === DEFAULT_BASE) document.body.dataset.sec = 'ai';
    render();
  }).catch(err => status(err, true));
  invoke('autostart_get').then(on => { autostart = !!on; render(); }).catch(() => {});
  invoke('prefs_request').catch(() => {});
})();
