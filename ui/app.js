'use strict';

(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  const $ = s => document.querySelector(s);
  const body = document.body;

  const answerEl = $('#answer');
  const messages = $('#messages');
  const input = $('#input');
  const titleEl = $('#title');
  const detailEl = $('#detail');
  const wave = $('#wave');
  const modelEl = $('#model');
  const ttsBtn = $('#ttsToggle');
  const mascot = $('#mascot');
  const artEl = $('#art');
  const miniArt = $('#miniArt');
  const progEl = $('#prog');
  const progFill = $('#progFill');
  const playIcon = $('#playIcon');
  const pageTitle = $('#pageTitle');
  const dotsEl = $('#dots');
  const clockEl = $('#clock');
  const icTime = $('#icTime');
  const icDate = $('#icDate');
  const battEl = $('#batt');
  const pinBtn = $('#pinBtn');
  const liveRing = $('#liveRing');
  const island = $('#island');

  const hudEl = $('#hud');
  const hudIcon = $('#hudIcon');
  const hudLabel = $('#hudLabel');
  const hudBar = $('#hudBar');
  const hudVal = $('#hudVal');

  const mTitle = $('#mTitle');
  const mArtist = $('#mArtist');
  const mApp = $('#mApp');
  const mPos = $('#mPos');
  const mLeft = $('#mLeft');
  const lyrBtn = $('#lyrBtn');
  const lyrList = $('#lyrList');
  const lyrStatus = $('#lyrStatus');
  const lyrNote = $('#lyrNote');

  const fRing = $('#fRing');
  const fTime = $('#fTime');
  const fPhase = $('#fPhase');
  const fCount = $('#fCount');
  const fStart = $('#fStart');

  const todoIn = $('#todoIn');
  const todoList = $('#todoList');
  const todoCount = $('#todoCount');
  const todoClear = $('#todoClear');

  const waterPg = $('.pg[data-p="water"]');
  const wBig = $('#wBig');
  const wPct = $('#wPct');
  const wBar = $('#wBar');
  const wChart = $('#wChart');
  const wDrink = $('#wDrink');
  const wUndo = $('#wUndo');
  const wSet = $('#wSet');
  const wGoal = $('#wGoal');
  const wCup = $('#wCup');
  const wEvery = $('#wEvery');

  const shelfPg = $('.pg[data-p="shelf"]');
  const shelfList = $('#shelfList');
  const shelfCount = $('#shelfCount');
  const shelfClear = $('#shelfClear');
  const shelfPaste = $('#shelfPaste');

  const PLAY_D = 'M8 5v14l11-7z';
  const PAUSE_D = 'M7 5h4v14H7zM13 5h4v14h-4z';

  const PAGE_ORDER = ['music', 'focus', 'todo', 'water', 'shelf', 'chat'];
  const PAGE_LABEL = {
    music: 'Musik',
    focus: 'Fokus',
    todo: 'Tugas',
    water: 'Minum',
    shelf: 'File',
    chat: 'Obrolan',
  };

  const SIZES = {
    idle: [168, 36],
    live: [260, 38],
    liveLyr: [320, 38],
    hud: [300, 40],
    alert: [340, 52],
    listening: [320, 60],
    thinking: [240, 44],
  };
  const OPEN = {
    music: [460, 190],
    lyrics: [620, 190],
    focus: [460, 176],
    todo: [460, 232],
    water: [460, 176],
    shelf: [460, 176],
    chat: [500, 392],
  };

  const POMODORO = { focus: 25 * 60, short: 5 * 60, long: 15 * 60 };
  const PHASE_LABEL = { focus: 'Fokus', short: 'Istirahat', long: 'Istirahat panjang' };

  const HUD_ICONS = {
    speaker: '<path d="M4 9h3l4-4v14l-4-4H4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M18 6a8 8 0 0 1 0 12" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>',
    muted: '<path d="M4 9h3l4-4v14l-4-4H4z"/><path d="m15 9 6 6M21 9l-6 6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    caps: '<path d="M12 4 4 12h5v4h6v-4h5z"/><rect x="7" y="18" width="10" height="2.2" rx="1.1"/>',
    power: '<rect x="3" y="7" width="15" height="10" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/><rect x="19" y="10" width="2.6" height="4" rx="1"/><path d="M6 10.5h3.5l-1.8 2.4h3L7.5 16.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>',
    battery: '<rect x="3" y="7" width="15" height="10" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.8"/><rect x="19" y="10" width="2.6" height="4" rx="1"/><rect x="5" y="9" width="8" height="6" rx="1"/>',
    keyboard: '<rect x="3" y="6" width="18" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M7 10h.01M10 10h.01M13 10h.01M16 10h.01M7 13h.01M17 13h.01M10 14h4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>',
    wifi: '<path d="M12 18h.01M5 11a10 10 0 0 1 14 0M8.5 14.5a5 5 0 0 1 7 0" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>',
    headphone: '<path d="M4 14v-2a8 8 0 0 1 16 0v2M4 14a2 2 0 0 0 2 2h1v-4H6a2 2 0 0 0-2 2zM20 14a2 2 0 0 1-2 2h-1v-4h1a2 2 0 0 1 2 2z" fill="none" stroke="currentColor" stroke-width="1.8"/>',
  };
  const FOLDER_SVG = '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>';
  const IMAGE_SVG = '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10" r="1.4"/><path d="m4 17 5-5 4 4 3-2 4 4"/>';

  // ---------- persisted state ----------
  function load(key, fallback) {
    try {
      const v = JSON.parse(localStorage.getItem(key));
      return v == null ? fallback : v;
    } catch (_) { return fallback; }
  }
  function save(key, value) {
    try { localStorage.setItem(key, JSON.stringify(value)); } catch (_) {}
  }

  let page = 'chat';
  let state = 'idle';
  let hover = false;
  let pinned = false;
  let dragging = false;
  let draggingOut = false;
  let compact = false;
  let ttsOn = true;
  let autoCollapseMs = 5000;
  let history = [];
  let pending = null;
  let answerText = '';
  let openTimer = 0;
  let leaveTimer = 0;
  let collapseTimer = 0;
  let hudTimer = 0;
  let wheelAt = 0;
  let dictating = false;
  let listening = false;
  let sleepTimer = 0;
  let eyesTimer = 0;
  let focusOnOpen = false;

  let chatWatch = 0;
  let listenWatch = 0;
  let dictWatch = 0;

  const activities = { timer: null, media: null, agent: null };
  let timerHandle = 0;

  const focus = Object.assign(
    { phase: 'focus', runningUntil: null, remaining: POMODORO.focus, count: 0 },
    load('blip.focus', {})
  );
  if (focus.runningUntil && focus.runningUntil <= Date.now()) {
    focus.runningUntil = null;
    focus.remaining = POMODORO[focus.phase] || POMODORO.focus;
  }

  let todos = load('blip.todos', []);
  if (!Array.isArray(todos)) todos = [];

  const water = Object.assign(
    { goal: 2000, cup: 250, start: 8, end: 22, every: 60, log: {}, lastNudge: 0 },
    load('blip.water', {})
  );
  if (!water.log || typeof water.log !== 'object') water.log = {};

  let shelf = load('blip.shelf', []);
  if (!Array.isArray(shelf)) shelf = [];

  // Lyrics state: status is idle|loading|synced|plain|none|error
  const lyr = { key: '', status: 'idle', hidden: '' };
  const lyricCache = new Map();
  let lrcLines = [];
  let lastLrc = -2;
  let lyrTimer = 0;
  let artSrc = '';

  function lyricsVisible() {
    return !!(
      lyr.key &&
      lyr.hidden !== lyr.key &&
      (lyr.status === 'synced' || lyr.status === 'plain')
    );
  }

  // ---------- helpers ----------
  function reportRect(w, h) {
    invoke('set_rect', { w, h }).catch(() => {});
  }

  // Native surface is opaque; clip it to the island's live shape every animation frame.
  let shapeKey = '';
  let shapeRaf = 0;
  function reportShape() {
    shapeRaf = 0;
    const cs = getComputedStyle(island);
    const w = island.offsetWidth, h = island.offsetHeight;
    const r = parseFloat(cs.borderBottomLeftRadius) || 0;
    const e = parseFloat(getComputedStyle(island, '::before').width) || 0;
    const key = w + ',' + h + ',' + r.toFixed(1) + ',' + e.toFixed(1);
    if (key === shapeKey) return;
    shapeKey = key;
    invoke('set_shape', { w, h, r, e }).catch(() => {});
  }
  function queueShape() {
    if (!shapeRaf) shapeRaf = requestAnimationFrame(reportShape);
  }
  new ResizeObserver(queueShape).observe(island);
  island.addEventListener('transitionrun', queueShape);
  island.addEventListener('transitionend', queueShape);

  function armSleep() {
    clearTimeout(sleepTimer);
    clearTimeout(eyesTimer);
    body.classList.remove('sleepy', 'eyesmode');
    if (state !== 'idle') return;
    eyesTimer = setTimeout(() => body.classList.add('eyesmode'), 8000);
    sleepTimer = setTimeout(() => body.classList.add('sleepy'), 28000);
  }

  function answerHeight() {
    return Math.min(60 + Math.max(answerEl.scrollHeight, 0), 220);
  }

  function visiblePages() {
    return PAGE_ORDER.filter(p => p !== 'music' || !!activities.media);
  }

  function applySize() {
    let w;
    let h;
    if (state === 'open') {
      [w, h] = OPEN[page === 'music' && lyricsVisible() ? 'lyrics' : page] || OPEN.chat;
    } else if (state === 'answer') {
      w = 460;
      h = answerHeight();
    } else if (compact && (state === 'idle' || state === 'live')) {
      [w, h] = [140, 6];
    } else if (state === 'live' && body.dataset.live === 'music' && lrcLines.length) {
      [w, h] = SIZES.liveLyr;
    } else {
      [w, h] = SIZES[state] || SIZES.idle;
    }
    body.style.setProperty('--w', w + 'px');
    body.style.setProperty('--h', h + 'px');
    body.style.setProperty('--rowh', (state === 'answer' ? 60 : h) + 'px');
    if (state === 'open') {
      body.style.setProperty('--ow', w + 'px');
      body.style.setProperty('--oh', h + 'px');
    }
    if (state === 'answer') body.style.setProperty('--ah', h + 'px');
    reportRect(w, h);
  }

  function go(next) {
    if (next === state) {
      applySize();
      return;
    }
    const wasOpen = state === 'open';
    state = next;
    body.dataset.state = next;
    if (next !== 'alert') delete body.dataset.tone;
    if (next === 'idle') mascot.style.transform = '';
    armSleep();

    if (next === 'open') {
      renderPage();
      if (page === 'chat') {
        if (focusOnOpen) invoke('focus_input').catch(() => {});
        if (!pending) renderHistory();
      }
      focusOnOpen = false;
    } else if (wasOpen && (page === 'chat' || page === 'todo')) {
      invoke('release_focus').catch(() => {});
    }

    if (next === 'answer') requestAnimationFrame(applySize);
    else applySize();
    if (next === 'live' || next === 'idle') applyActivityLabel();
    renderDots();
  }

  function setTitle(t, d) {
    titleEl.textContent = t || 'Blip';
    detailEl.textContent = d || '';
  }

  function addMsg(kind, text) {
    const el = document.createElement('div');
    el.className = 'msg ' + kind;
    el.textContent = text;
    messages.appendChild(el);
    messages.scrollTop = messages.scrollHeight;
    return el;
  }

  function renderHistory() {
    messages.textContent = '';
    for (const m of history) addMsg(m.role === 'user' ? 'user' : 'bot', m.content);
  }

  function markPage() {
    document.querySelectorAll('.pg').forEach(el => {
      if (el.dataset.p === page) {
        el.classList.add('cur');
        el.classList.remove('out-l', 'out-r');
      } else {
        el.classList.remove('cur', 'out-l', 'out-r');
      }
    });
  }

  function renderDots() {
    const vis = visiblePages();
    if (!vis.includes(page)) {
      setPage(vis.includes('chat') ? 'chat' : vis[0]);
      return;
    }
    body.dataset.page = page;
    dotsEl.textContent = '';
    for (const id of vis) {
      const d = document.createElement('i');
      d.dataset.p = id;
      if (id === page) d.className = 'on';
      d.addEventListener('click', () => setPage(id));
      dotsEl.appendChild(d);
    }
    pageTitle.textContent = PAGE_LABEL[page] || '';
  }

  function setPage(id, dir) {
    const vis = visiblePages();
    if (!vis.includes(id)) id = vis.includes('chat') ? 'chat' : vis[0];
    if (!id || id === page) {
      renderDots();
      return;
    }
    const was = page;
    if (dir === undefined) {
      const oldIdx = PAGE_ORDER.indexOf(was);
      const newIdx = PAGE_ORDER.indexOf(id);
      dir = newIdx >= oldIdx ? 1 : -1;
    }
    const oldEl = document.querySelector(`.pg[data-p="${was}"]`);
    const newEl = document.querySelector(`.pg[data-p="${id}"]`);

    if (oldEl) {
      oldEl.classList.add(dir > 0 ? 'out-l' : 'out-r');
      oldEl.classList.remove('cur');
      setTimeout(() => {
        oldEl.classList.remove('out-l', 'out-r');
      }, 320);
    }
    if (newEl) {
      newEl.style.setProperty('--dx', (dir > 0 ? 24 : -24) + 'px');
      void newEl.offsetWidth;
      newEl.classList.add('cur');
    }

    page = id;
    body.dataset.page = id;
    save('blip.page', id);
    if (state === 'open') {
      if (was === 'chat' || was === 'todo') invoke('release_focus').catch(() => {});
      if (id === 'chat') {
        invoke('focus_input').catch(() => {});
        if (!pending) renderHistory();
        setTimeout(() => input.focus(), 120);
      }
      renderPage();
      applySize();
    }
    renderDots();
  }

  function stepPage(dir) {
    const vis = visiblePages();
    if (vis.length < 2) return;
    const i = vis.indexOf(page);
    setPage(vis[(i + dir + vis.length) % vis.length], dir);
  }

  // ---------- live activities ----------
  function focusLeft() {
    if (focus.runningUntil) return Math.max(0, Math.ceil((focus.runningUntil - Date.now()) / 1000));
    return Math.max(0, focus.remaining);
  }

  function updateActivityRow() {
    if (['listening', 'thinking', 'alert', 'answer', 'open', 'hud'].includes(state)) return;
    const next = topActivity() ? 'live' : 'idle';
    if (next === state) applyActivityLabel();
    else go(next);
  }

  function applyActivityLabel() {
    const top = topActivity();
    body.dataset.live = top ? top.kind : '';
    if (top) {
      let detail = top.detail;
      if (top.kind === 'music' && lrcLines.length) {
        const m = activities.media;
        const idx = m ? lrcIndex(lrcLines, trackPos(m, Date.now())) : -1;
        detail = idx >= 0 && lrcLines[idx] ? lrcLines[idx].line : top.detail || '';
      }
      setTitle(top.title, detail);
    } else {
      setTitle('Blip', '');
    }
    if (top && top.kind === 'focus' && focus.runningUntil) {
      const total = POMODORO[focus.phase] || 1;
      liveRing.style.setProperty('--p', ((1 - focusLeft() / total) * 100).toFixed(1));
    }
  }

  function topActivity() {
    if (activities.agent && activities.agent.state === 'waiting') {
      return { kind: 'agent', title: activities.agent.agent, detail: activities.agent.text || 'menunggu' };
    }
    if (activities.timer) {
      const left = Math.max(0, Math.ceil((activities.timer.end - Date.now()) / 1000));
      return { kind: 'timer', title: 'timer', detail: fmt(left) };
    }
    if (activities.agent && activities.agent.state === 'working') {
      return { kind: 'agent', title: activities.agent.agent, detail: activities.agent.text || 'bekerja...' };
    }
    if (focus.runningUntil) {
      return { kind: 'focus', title: focus.phase === 'focus' ? 'Fokus' : 'Istirahat', detail: fmt(focusLeft()) };
    }
    if (activities.media && activities.media.playing) {
      return { kind: 'music', title: activities.media.title || 'musik', detail: activities.media.artist || '' };
    }
    return null;
  }

  // ---------- timer ----------
  function setTimer(secs) {
    clearInterval(timerHandle);
    activities.timer = { end: Date.now() + secs * 1000 };
    timerHandle = setInterval(() => {
      if (!activities.timer) return clearInterval(timerHandle);
      if (Date.now() >= activities.timer.end) {
        clearInterval(timerHandle);
        activities.timer = null;
        alertState('timer selesai', 6000);
        invoke('beep').catch(() => {});
      } else if (state === 'live' || state === 'idle') {
        updateActivityRow();
      }
    }, 1000);
    alertState('timer ' + fmt(secs) + ' dipasang', 2000);
  }

  function alertState(text, ms, tone) {
    setTitle(text, '');
    body.dataset.tone = tone || '';
    go('alert');
    clearTimeout(collapseTimer);
    collapseTimer = setTimeout(() => {
      if (state === 'alert') go(topActivity() ? 'live' : 'idle');
    }, ms);
  }

  // ---------- hover / open ----------
  function openNotch(force, focusInput) {
    clearTimeout(openTimer);
    focusOnOpen = !!focusInput;
    if (state === 'open') {
      if (force) setPage(force);
      return;
    }
    const vis = visiblePages();
    let next = force;
    if (!next) {
      if (state === 'answer' || pending || state === 'thinking') next = 'chat';
      else if (activities.media && activities.media.playing) next = 'music';
      else if (focus.runningUntil) next = 'focus';
      else next = load('blip.page', 'chat');
    }
    if (!vis.includes(next)) next = vis.includes('chat') ? 'chat' : vis[0];
    page = next;
    body.dataset.page = next;
    save('blip.page', next);
    markPage();
    go('open');
  }

  function inputBusy() {
    const active = document.activeElement;
    if (page === 'chat' && (pending || listening)) return true;
    if (page === 'chat' && active === input && input.value.trim()) return true;
    if (page === 'todo' && active === todoIn && todoIn.value.trim()) return true;
    return false;
  }

  function maybeClose() {
    if (state !== 'open' || hover || inputBusy() || pinned || dragging || draggingOut) return;
    if (page === 'chat' || page === 'todo') invoke('release_focus').catch(() => {});
    go(topActivity() ? 'live' : 'idle');
  }

  listen('hover', e => {
    hover = !!e.payload;
    if (hover) {
      clearTimeout(leaveTimer);
      if (body.classList.contains('sleepy')) {
        body.classList.remove('sleepy');
        body.classList.add('wake');
        setTimeout(() => body.classList.remove('wake'), 600);
      }
      if (['idle', 'live', 'hud', 'answer', 'alert', 'thinking'].includes(state)) {
        clearTimeout(openTimer);
        openTimer = setTimeout(() => openNotch(), 70);
      }
      if (state === 'alert') {
        clearTimeout(collapseTimer);
      }
    } else {
      clearTimeout(openTimer);
      if (state === 'open') {
        clearTimeout(leaveTimer);
        leaveTimer = setTimeout(maybeClose, 300);
      } else if (state === 'alert') {
        clearTimeout(collapseTimer);
        collapseTimer = setTimeout(() => {
          if (state === 'alert') go(topActivity() ? 'live' : 'idle');
        }, 1200);
      }
    }
  });

  listen('cursor', e => {
    const r = mascot.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const lim = state === 'idle' ? 2 : 3;
    const dx = Math.max(-lim, Math.min(lim, (e.payload.x - cx) / 14));
    const dy = Math.max(-lim, Math.min(lim, (e.payload.y - cy) / 14));
    mascot.style.transform = 'translate(' + dx + 'px,' + dy + 'px)';
  });

  listen('compact', e => {
    compact = !!e.payload;
    body.classList.toggle('compact', compact);
    applySize();
  });

  pinBtn.addEventListener('click', () => {
    pinned = !pinned;
    pinBtn.classList.toggle('on', pinned);
  });

  // ---------- chat ----------
  function runIntent(it) {
    let label = '';
    let call;
    if (it.type === 'media') {
      call = invoke('media_cmd', { cmd: it.cmd });
      label = INTENT_LABEL[it.cmd];
    } else if (it.type === 'volume') {
      if (it.dir === 'set') {
        call = invoke('volume', { dir: 'set', level: it.level });
        label = 'volume ' + it.level + '%';
      } else {
        call = invoke('volume', { dir: it.dir, steps: it.steps });
        label = INTENT_LABEL[it.dir];
      }
    } else if (it.type === 'open') {
      call = invoke('open_target', { target: it.target });
      label = 'membuka ' + it.label;
    } else if (it.type === 'todo') {
      addTodo(it.text);
      label = INTENT_LABEL.todo;
    } else if (it.type === 'water') {
      drink();
      label = '+' + water.cup + ' ml dicatat';
    } else if (it.type === 'focus') {
      if (it.cmd === 'start') focusStart(); else focusStop();
      label = it.cmd === 'start' ? INTENT_LABEL.focusStart : INTENT_LABEL.focusStop;
    }
    alertState(label || 'ok', 1600, 'ok');
    if (call) Promise.resolve(call).catch(err => alertState('gagal: ' + err, 3000));
  }

  function send(text) {
    text = (text || '').trim();
    if (!text || pending) return;
    clearTimeout(collapseTimer);
    const open = state === 'open';
    const intent = parseIntent(text);
    if (intent) {
      addMsg('user', text);
      input.value = '';
      input.style.height = 'auto';
      runIntent(intent);
      return;
    }
    const secs = parseTimer(text);
    if (secs) {
      addMsg('user', text);
      history.push({ role: 'user', content: text });
      input.value = '';
      input.style.height = 'auto';
      setTimer(secs);
      return;
    }
    addMsg('user', text);
    history.push({ role: 'user', content: text });
    input.value = '';
    input.style.height = 'auto';
    answerText = '';
    answerEl.textContent = '';
    answerEl.classList.remove('err');
    if (!open) {
      setTitle('Blip', 'berpikir...');
      go('thinking');
    }
    const el = addMsg('bot', '');
    el.innerHTML = '<span class="caret"></span>';
    const id = 'a' + Date.now();
    pending = { id, el, text: '' };

    clearTimeout(chatWatch);
    chatWatch = setTimeout(() => {
      if (pending && pending.id === id && !pending.text) {
        invoke('chat_abort', { id }).catch(() => {});
        onChatError({ id, error: 'waktu habis' });
      }
    }, 45000);

    invoke('chat_send', { id, messages: history }).catch(err => onChatError({ id, error: String(err) }));
  }

  listen('chat:chunk', e => {
    if (!pending || e.payload.id !== pending.id) return;
    clearTimeout(chatWatch);
    pending.text += e.payload.delta;
    if (pending.el) pending.el.textContent = pending.text;
    messages.scrollTop = messages.scrollHeight;
    if (state === 'open') return;
    answerText = pending.text;
    answerEl.textContent = answerText;
    answerEl.appendChild(caret());
    if (state !== 'answer') go('answer');
    answerEl.scrollTop = answerEl.scrollHeight;
    requestAnimationFrame(applySize);
  });

  function caret() {
    const c = document.createElement('span');
    c.className = 'caret';
    return c;
  }

  listen('chat:done', e => {
    if (!pending || e.payload.id !== pending.id) return;
    clearTimeout(chatWatch);
    const text = pending.text.trim();
    if (pending.el) pending.el.textContent = text || '(kosong)';
    history.push({ role: 'assistant', content: text });
    pending = null;
    answerEl.textContent = text;
    if (e.payload.aborted) {
      if (text && state !== 'open') go('answer');
      return;
    }
    if (state === 'open') {
      if (!hover) setTimeout(maybeClose, 1500);
      if (text && ttsOn) invoke('speak', { text }).catch(() => {});
      return;
    }
    if (text && ttsOn) invoke('speak', { text }).catch(() => scheduleCollapse());
    else scheduleCollapse();
    if (state !== 'answer') go('answer');
  });

  listen('chat:error', e => {
    clearTimeout(chatWatch);
    onChatError(e.payload);
  });

  function onChatError(p) {
    if (!pending || p.id !== pending.id) return;
    clearTimeout(chatWatch);
    const el = pending.el;
    pending = null;
    answerEl.classList.add('err');
    answerEl.textContent = 'gagal: ' + p.error;
    if (el) {
      el.classList.add('err');
      el.textContent = 'gagal: ' + p.error;
    }
    if (state === 'open') {
      setTitle('Blip', 'gagal');
      if (!hover) setTimeout(maybeClose, 2500);
      return;
    }
    setTitle('Blip', 'gagal');
    go('answer');
    scheduleCollapse(7000);
  }

  function scheduleCollapse(ms) {
    clearTimeout(collapseTimer);
    collapseTimer = setTimeout(() => {
      if (hover || state === 'open') return;
      if (state === 'answer' || state === 'alert' || state === 'live' || state === 'idle') {
        go(topActivity() ? 'live' : 'idle');
      }
    }, ms || autoCollapseMs);
  }

  listen('tts:start', () => {
    body.classList.add('talking');
    if (state === 'answer') setTitle('Blip', 'bicara...');
  });
  listen('tts:end', () => {
    body.classList.remove('talking');
    if (state === 'answer') {
      setTitle('Blip', '');
      scheduleCollapse();
    }
  });

  // ---------- stt ----------
  function startListening() {
    if (listening) {
      stopListening();
      return;
    }
    if (pending) invoke('chat_abort', { id: pending.id }).catch(() => {});
    invoke('stop_speak').catch(() => {});
    listening = true;
    setTitle('Blip', 'mendengar...');
    go('listening');

    clearTimeout(listenWatch);
    listenWatch = setTimeout(() => {
      if (listening) stopListening();
    }, 30000);

    invoke('rec_start', { mode: 'chat' }).catch(err => {
      clearTimeout(listenWatch);
      listening = false;
      setTitle('Blip', 'mic gagal');
      answerEl.classList.add('err');
      answerEl.textContent = 'mic gagal: ' + err;
      go('answer');
      scheduleCollapse(4000);
    });
  }

  function stopListening() {
    if (!listening) return;
    clearTimeout(listenWatch);
    listening = false;
    setTitle('Blip', 'memproses...');
    go('thinking');
    invoke('rec_stop').catch(() => {});
  }

  listen('rec:level', e => {
    if (state !== 'listening') return;
    const rms = Math.max(0, Math.min(1, Number(e.payload) * 8));
    const bars = wave.children;
    for (let i = 0; i < bars.length; i++) {
      bars[i].style.height = (4 + rms * (14 + (i % 3) * 4)) + 'px';
    }
  });

  listen('stt:result', e => {
    clearTimeout(listenWatch);
    listening = false;
    send(e.payload.text);
  });

  listen('stt:empty', () => {
    clearTimeout(listenWatch);
    listening = false;
    setTitle('Blip', 'tidak terdengar');
    go('live');
    scheduleCollapse(1500);
  });

  listen('stt:error', e => {
    clearTimeout(listenWatch);
    listening = false;
    answerEl.classList.add('err');
    answerEl.textContent = e.payload.error;
    setTitle('Blip', 'stt gagal');
    go('answer');
    scheduleCollapse(4000);
  });

  // ---------- dictate ----------
  listen('dictate', e => {
    if (e.payload === 'start') {
      dictating = true;
      setTitle('Blip', 'dikte...');
      go('listening');
      clearTimeout(dictWatch);
      dictWatch = setTimeout(() => {
        if (dictating) {
          dictating = false;
          listening = false;
          go('idle');
        }
      }, 60000);
    } else if (e.payload === 'done') {
      clearTimeout(dictWatch);
      dictating = false;
      listening = false;
      setTitle('Blip', '');
      go('idle');
    }
  });

  // ---------- hotkey ----------
  listen('hotkey', () => {
    if (state === 'listening') {
      if (dictating) return;
      stopListening();
      return;
    }
    if (state === 'thinking') {
      if (pending) invoke('chat_abort', { id: pending.id }).catch(() => {});
      pending = null;
      go('idle');
      return;
    }
    if (state === 'answer' || state === 'open') {
      invoke('stop_speak').catch(() => {});
      startListening();
      return;
    }
    startListening();
  });

  listen('ui:show', () => {
    if (state === 'idle' || state === 'live') startListening();
  });

  listen('ui:hide', () => hideAll());
  listen('ui:ask', e => {
    const text = (e.payload && e.payload.text) || '';
    if (!parseIntent(text)) {
      if (state !== 'open') openNotch('chat', true);
      else if (page !== 'chat') setPage('chat');
      else invoke('focus_input').catch(() => {});
      setTimeout(() => input.focus(), 120);
    }
    send(text);
  });

  function hideAll() {
    pinned = false;
    pinBtn.classList.remove('on');
    invoke('stop_speak').catch(() => {});
    if (listening) {
      listening = false;
      invoke('rec_stop').catch(() => {});
    }
    if (pending) invoke('chat_abort', { id: pending.id }).catch(() => {});
    pending = null;
    clearTimeout(collapseTimer);
    if (page === 'chat' || page === 'todo') invoke('release_focus').catch(() => {});
    input.blur();
    go(topActivity() ? 'live' : 'idle');
  }

  // ---------- music ----------
  listen('media', e => {
    const m = e.payload;
    const prevTrack = activities.media
      ? activities.media.title + '|' + activities.media.artist + '|' + (activities.media.app || '')
      : '';
    if (!m.title && !m.playing) {
      activities.media = null;
    } else {
      const art = 'art' in m ? m.art : (activities.media && activities.media.art) || null;
      activities.media = Object.assign({}, m, { art, at: Date.now() });
    }
    const newTrack = activities.media
      ? activities.media.title + '|' + activities.media.artist + '|' + (activities.media.app || '')
      : '';
    if (newTrack !== prevTrack) {
      clearTimeout(lyrTimer);
      lyrTimer = setTimeout(fetchLyrics, 400);
    }
    renderMusic();
    if (!visiblePages().includes(page)) setPage('chat');
    if (state === 'idle' || state === 'live' || state === 'hud') updateActivityRow();
    if (state === 'open' && page === 'music') {
      applySize();
      tickMusic();
    }
    renderDots();
  });

  function renderMusic() {
    const m = activities.media;
    mTitle.textContent = m ? m.title || 'musik' : '';
    mArtist.textContent = m ? m.artist || '' : '';
    mApp.textContent = m ? appName(m.app) : '';
    for (const img of [artEl, miniArt]) {
      if (m && m.art) {
        if (img.src !== m.art) img.src = m.art;
      } else {
        img.removeAttribute('src');
      }
    }
    const src = (m && m.art) || '';
    if (src !== artSrc) {
      artSrc = src;
      artTint(src);
    }
    body.classList.toggle('has-art', !!(m && m.art));
    body.classList.toggle('paused', !(m && m.playing));
    playIcon.setAttribute('d', m && m.playing ? PAUSE_D : PLAY_D);
  }

  function artTint(src) {
    if (!src) {
      body.style.removeProperty('--art');
      return;
    }
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = 8;
      c.height = 8;
      const ctx = c.getContext('2d', { willReadFrequently: true });
      if (!ctx) return;
      ctx.drawImage(img, 0, 0, 8, 8);
      const rgb = avgColor(ctx.getImageData(0, 0, 8, 8).data);
      if (rgb) body.style.setProperty('--art', 'rgb(' + rgb.join(',') + ')');
      else body.style.removeProperty('--art');
    };
    img.src = src;
  }

  function tickMusic() {
    const m = activities.media;
    if (!m) return;
    if (state === 'open' && page === 'music') {
      const p = trackPos(m, Date.now());
      progFill.style.width = (m.duration ? (p / m.duration) * 100 : 0) + '%';
      mPos.textContent = fmtTrack(p);
      mLeft.textContent = '-' + fmtTrack((m.duration || 0) - p);
      tickLyrics();
    } else if (state === 'live') {
      tickLyrics();
      applyActivityLabel();
    }
  }

  setInterval(tickMusic, 250);

  progEl.addEventListener('click', ev => {
    ev.stopPropagation();
    const m = activities.media;
    if (!m || !m.duration) return;
    const r = progEl.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (ev.clientX - r.left) / r.width));
    seekTo(ratio * m.duration);
  });

  for (const b of document.querySelectorAll('#media button[data-cmd]')) {
    b.addEventListener('click', ev => {
      ev.stopPropagation();
      const m = activities.media;
      if (b.dataset.cmd === 'toggle' && m) {
        m.position = trackPos(m, Date.now());
        m.at = Date.now();
        m.playing = !m.playing;
        renderMusic();
      }
      invoke('media_cmd', { cmd: b.dataset.cmd }).catch(() => {});
    });
  }

  function seekTo(pos) {
    const m = activities.media;
    if (!m) return;
    m.position = pos;
    m.at = Date.now();
    tickMusic();
    invoke('media_cmd', { cmd: 'seek', pos }).catch(() => {});
  }

  // ---------- lyrics ----------
  function fetchLyrics() {
    const m = activities.media;
    if (!m || !m.title) {
      lyr.key = '';
      lyr.status = 'idle';
      applyLyrics(null);
      syncLyrClass();
      return;
    }
    const p = parseTrack(m.title, m.artist);
    const key = normKey(p.track) + '|' + normKey(p.artist);
    if (lyricCache.has(key)) {
      const res = lyricCache.get(key);
      lyr.key = key;
      lyr.status = res && res.synced ? 'synced' : res && res.plain ? 'plain' : 'none';
      applyLyrics(res);
      syncLyrClass();
      if (state === 'open' || state === 'live') applySize();
      return;
    }

    lyr.key = key;
    lyr.status = 'loading';
    applyLyrics(null);
    syncLyrClass();
    if (state === 'open' || state === 'live') applySize();

    const title = p.track;
    const artist = p.artist;
    const duration = m.duration || 0;
    invoke('lyrics', { title, artist, duration })
      .then(res => {
        lyricCache.set(key, res);
        if (lyricCache.size > 60) {
          lyricCache.delete(lyricCache.keys().next().value);
        }
        if (lyr.key === key) {
          lyr.status = res && res.synced ? 'synced' : res && res.plain ? 'plain' : 'none';
          applyLyrics(res);
          syncLyrClass();
          if (state === 'open' || state === 'live') applySize();
        }
      })
      .catch(() => {
        if (lyr.key === key) {
          lyr.status = 'error';
          applyLyrics(null);
          syncLyrClass();
          if (state === 'open' || state === 'live') applySize();
        }
      });
  }

  function syncLyrClass() {
    body.classList.toggle('lyr', lyricsVisible());
    lyrBtn.classList.toggle('loading', lyr.status === 'loading');
    if (lyr.status === 'loading') lyrNote.textContent = 'mencari lirik…';
    else if (lyr.status === 'none') lyrNote.textContent = 'lirik tidak ditemukan';
    else if (lyr.status === 'error')
      lyrNote.textContent = 'lirik gagal dimuat · klik ikon untuk coba lagi';
    else lyrNote.textContent = '';
  }

  lyrBtn.addEventListener('click', () => {
    if (lyricsVisible()) {
      lyr.hidden = lyr.key;
    } else if (lyr.status === 'none' || lyr.status === 'error') {
      lyricCache.delete(lyr.key);
      lyr.hidden = '';
      fetchLyrics();
    } else {
      lyr.hidden = '';
    }
    syncLyrClass();
    applySize();
  });

  function applyLyrics(res) {
    lrcLines = res && res.synced ? parseLrc(res.synced) : [];
    lyrList.textContent = '';
    lastLrc = -2;
    const lines = lrcLines.length
      ? lrcLines.map(x => x.line)
      : res && res.plain
      ? res.plain.split(/\r?\n/).filter(Boolean)
      : [];
    if (!lines.length) {
      lyrStatus.textContent = lyr.status === 'loading' ? 'mencari lirik…' : '';
      return;
    }
    lyrStatus.textContent = '';
    lines.forEach((line, i) => {
      const li = document.createElement('li');
      li.textContent = line;
      if (lrcLines.length) {
        li.addEventListener('click', () => seekTo(lrcLines[i].t));
      }
      lyrList.appendChild(li);
    });
    tickLyrics();
  }

  function tickLyrics() {
    if (!lrcLines.length || !lyrList.children.length) return;
    const m = activities.media;
    if (!m) return;
    const idx = lrcIndex(lrcLines, trackPos(m, Date.now()));
    if (idx === lastLrc) return;
    lastLrc = idx;
    for (let i = 0; i < lyrList.children.length; i++) {
      lyrList.children[i].classList.toggle('on', i === idx);
    }
    if (idx >= 0) lyrList.scrollTo({ top: Math.max(0, idx * 23 - 30), behavior: 'smooth' });
  }

  // ---------- focus ----------
  function saveFocus() {
    save('blip.focus', focus);
  }

  function renderFocus() {
    const left = focusLeft();
    const total = POMODORO[focus.phase] || POMODORO.focus;
    const p = Math.max(0, Math.min(100, (1 - left / total) * 100));
    fRing.style.setProperty('--p', p.toFixed(1));
    fTime.textContent = fmt(left);
    fPhase.textContent = PHASE_LABEL[focus.phase] || 'Fokus';
    fCount.textContent = 'sesi ' + Math.min(focus.count + 1, 4) + '/4';
    fStart.textContent = focus.runningUntil ? 'Jeda' : left < total ? 'Lanjut' : 'Mulai';
    body.classList.toggle('focusing', !!focus.runningUntil);
    if (focus.runningUntil) liveRing.style.setProperty('--p', p.toFixed(1));

    document.querySelectorAll('.chips button[data-ph]').forEach(b => {
      b.classList.toggle('on', b.dataset.ph === focus.phase);
    });
  }

  document.querySelectorAll('.chips button[data-ph]').forEach(b => {
    b.addEventListener('click', () => {
      const ph = b.dataset.ph;
      focus.phase = ph;
      focus.runningUntil = null;
      focus.remaining = POMODORO[ph];
      saveFocus();
      renderFocus();
      if (state === 'live') updateActivityRow();
    });
  });

  function focusStart() {
    if (focus.runningUntil) return;
    if (focus.remaining <= 0) focus.remaining = POMODORO[focus.phase];
    focus.runningUntil = Date.now() + focus.remaining * 1000;
    saveFocus();
    renderFocus();
    if (state === 'idle' || state === 'live') updateActivityRow();
  }

  function focusStop() {
    if (!focus.runningUntil) return;
    focus.remaining = focusLeft();
    focus.runningUntil = null;
    saveFocus();
    renderFocus();
    if (state === 'live') updateActivityRow();
  }

  function focusReset() {
    focus.runningUntil = null;
    focus.remaining = POMODORO[focus.phase];
    saveFocus();
    renderFocus();
    if (state === 'live') updateActivityRow();
  }

  function completeFocus() {
    const was = focus.phase;
    focus.runningUntil = null;
    if (was === 'focus') {
      focus.count += 1;
      focus.phase = focus.count % 4 === 0 ? 'long' : 'short';
    } else {
      focus.phase = 'focus';
    }
    focus.remaining = POMODORO[focus.phase];
    saveFocus();
    invoke('beep').catch(() => {});
    alertState(was === 'focus' ? 'waktunya istirahat' : 'waktunya fokus', 5000, 'ok');
    renderFocus();
  }

  function focusSkip() {
    focus.runningUntil = null;
    focus.remaining = 0;
    completeFocus();
  }

  fStart.addEventListener('click', () => (focus.runningUntil ? focusStop() : focusStart()));
  $('#fReset').addEventListener('click', focusReset);
  $('#fSkip').addEventListener('click', focusSkip);

  setInterval(() => {
    if (focus.runningUntil && focusLeft() <= 0) {
      completeFocus();
      return;
    }
    if (state === 'open' && page === 'focus') renderFocus();
    if (state === 'live' && body.dataset.live === 'focus') updateActivityRow();
  }, 1000);

  // ---------- to-do ----------
  function saveTodos() {
    save('blip.todos', todos);
  }

  function addTodo(text) {
    const parsed = parseTodo(text, Date.now());
    if (!parsed.text) return;
    todos.unshift({
      id: Date.now().toString(36) + Math.random().toString(36).slice(2, 6),
      text: parsed.text,
      done: false,
      dueAt: parsed.dueAt,
      notified: false,
    });
    saveTodos();
    renderTodos();
  }

  function dueText(ms) {
    const d = new Date(ms);
    const hm = String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
    const today = new Date();
    return d.toDateString() === today.toDateString() ? hm : 'besok ' + hm;
  }

  function renderTodos() {
    const today = dayKey(Date.now());
    const doneToday = todos.filter(t => t.done && t.doneAt && dayKey(t.doneAt) === today).length;
    const left = todos.filter(t => !t.done).length;
    todoCount.textContent = doneToday + ' selesai hari ini · ' + left + ' tersisa';
    todoList.textContent = '';
    if (!todos.length) {
      const e = document.createElement('div');
      e.className = 'empty';
      e.textContent = 'Belum ada tugas.';
      todoList.appendChild(e);
      return;
    }
    for (const t of todos) {
      const row = document.createElement('div');
      row.className = 'trow' + (t.done ? ' done' : '');
      const chk = document.createElement('button');
      chk.className = 'chk';
      chk.title = t.done ? 'Tandai belum' : 'Tandai selesai';
      chk.addEventListener('click', () => {
        t.done = !t.done;
        t.doneAt = t.done ? Date.now() : null;
        if (t.done) t.notified = true;
        saveTodos();
        renderTodos();
      });

      const tx = document.createElement('span');
      tx.className = 'tx';
      tx.textContent = t.text;
      tx.title = t.text + ' (klik dua kali untuk edit)';
      tx.addEventListener('dblclick', () => {
        const inp = document.createElement('input');
        inp.className = 'tin';
        inp.value = t.text;
        tx.replaceWith(inp);
        inp.focus();
        let committed = false;
        const commit = () => {
          if (committed) return;
          committed = true;
          const v = inp.value.trim();
          if (!v) {
            todos = todos.filter(x => x !== t);
          } else {
            t.text = v;
          }
          saveTodos();
          renderTodos();
        };
        inp.addEventListener('keydown', e => {
          if (e.key === 'Enter') {
            e.preventDefault();
            commit();
          } else if (e.key === 'Escape') {
            committed = true;
            renderTodos();
          }
        });
        inp.addEventListener('blur', commit);
      });

      row.append(chk, tx);
      if (t.dueAt) {
        const due = document.createElement('span');
        const late = !t.done && t.dueAt < Date.now();
        const soon = !t.done && !late && t.dueAt - Date.now() <= 3600000;
        due.className = 'due' + (late ? ' late' : soon ? ' soon' : '');
        due.textContent = dueText(t.dueAt);
        row.append(due);
      }
      const del = document.createElement('button');
      del.className = 'del';
      del.textContent = '×';
      del.title = 'Hapus';
      del.addEventListener('click', () => {
        todos = todos.filter(x => x !== t);
        saveTodos();
        renderTodos();
      });
      row.append(del);
      todoList.appendChild(row);
    }
  }

  todoIn.addEventListener('keydown', e => {
    if (e.key === 'Enter') {
      e.preventDefault();
      addTodo(todoIn.value);
      todoIn.value = '';
    }
  });
  todoIn.addEventListener('pointerdown', () => invoke('focus_input').catch(() => {}));
  $('#todoAdd').addEventListener('click', () => {
    addTodo(todoIn.value);
    todoIn.value = '';
    todoIn.focus();
  });
  todoClear.addEventListener('click', () => {
    todos = todos.filter(t => !t.done);
    saveTodos();
    renderTodos();
  });

  setInterval(() => {
    const now = Date.now();
    let changed = false;
    for (const t of todos) {
      if (!t.done && t.dueAt && !t.notified && now >= t.dueAt) {
        t.notified = true;
        changed = true;
        invoke('beep').catch(() => {});
        alertState('⏰ ' + t.text, 6000, 'ok');
      }
    }
    if (changed) {
      saveTodos();
      renderTodos();
    } else if (state === 'open' && page === 'todo' && !todoList.querySelector('.tin')) {
      // Refresh the soon/late badges, but never yank the inline editor out from under the user.
      renderTodos();
    }
  }, 20000);

  // ---------- water ----------
  function todayWater() {
    return water.log[dayKey(Date.now())] || [];
  }

  function waterTotal() {
    return todayWater().reduce((a, e) => a + (Number(e.ml) || 0), 0);
  }

  function saveWater() {
    const keys = Object.keys(water.log).sort().reverse();
    for (const k of keys.slice(14)) delete water.log[k];
    save('blip.water', water);
  }

  function drink() {
    const k = dayKey(Date.now());
    (water.log[k] = water.log[k] || []).push({ t: Date.now(), ml: water.cup });
    saveWater();
    renderWater();
  }

  function renderWater() {
    const total = waterTotal();
    wBig.textContent = (total / 1000).toFixed(2) + ' / ' + (water.goal / 1000).toFixed(1) + ' L';
    const pct = water.goal ? Math.min(100, Math.round((total / water.goal) * 100)) : 0;
    wPct.textContent = pct >= 100 ? 'Target tercapai' : pct + '% hari ini';
    wBar.style.width = pct + '%';
    const buckets = waterBuckets(todayWater(), water.start, water.end);
    const max = Math.max(water.goal / (buckets.length || 1), ...buckets, 1);
    wChart.textContent = '';
    for (const v of buckets) {
      const bar = document.createElement('i');
      bar.style.height = Math.max(2, (v / max) * 100) + '%';
      wChart.appendChild(bar);
    }
    wGoal.value = water.goal;
    wCup.value = water.cup;
    wEvery.value = water.every;
  }

  wDrink.addEventListener('click', drink);
  wUndo.addEventListener('click', () => {
    const entries = water.log[dayKey(Date.now())];
    if (entries && entries.length) {
      entries.pop();
      saveWater();
      renderWater();
    }
  });
  wSet.addEventListener('click', () => {
    waterPg.classList.toggle('set');
    if (waterPg.classList.contains('set')) renderWater();
  });

  function bindWaterInput(el, key, min, max) {
    el.addEventListener('change', () => {
      const v = Math.round(Number(el.value));
      if (!isFinite(v)) {
        el.value = water[key];
        return;
      }
      water[key] = Math.max(min, Math.min(max, v));
      el.value = water[key];
      saveWater();
      renderWater();
    });
  }
  bindWaterInput(wGoal, 'goal', 500, 6000);
  bindWaterInput(wCup, 'cup', 50, 1000);
  bindWaterInput(wEvery, 'every', 15, 240);

  setInterval(() => {
    const now = Date.now();
    const h = new Date(now).getHours() + new Date(now).getMinutes() / 60;
    if (h < water.start || h >= water.end) return;
    if (waterTotal() >= water.goal) return;
    const entries = todayWater();
    const lastDrink = entries.length ? Math.max(...entries.map(e => e.t)) : 0;
    const start = new Date(now);
    start.setHours(water.start, 0, 0, 0);
    const base = Math.max(lastDrink, water.lastNudge || 0, start.getTime());
    if (now - base >= water.every * 60000) {
      water.lastNudge = now;
      saveWater();
      invoke('beep').catch(() => {});
      alertState('waktunya minum air', 6000, 'ok');
    }
  }, 60000);

  // ---------- shelf ----------
  let shelfToastTimer = 0;
  function toastShelf(msg) {
    clearTimeout(shelfToastTimer);
    shelfCount.textContent = msg;
    shelfToastTimer = setTimeout(() => {
      shelfCount.textContent = shelf.length ? shelf.length + ' item' : '';
    }, 1200);
  }

  function saveShelf() {
    save('blip.shelf', shelf);
  }

  function addShelf(info, markNew = false) {
    if (!info || !info.length) return;
    const seen = new Set();
    const newPaths = new Set(info.map(x => x.path));
    shelf = info
      .concat(shelf)
      .filter(x => (seen.has(x.path) ? false : (seen.add(x.path), true)))
      .slice(0, 30);
    saveShelf();
    renderShelf(markNew ? newPaths : null);
  }

  function renderShelf(newPaths) {
    shelfPg.classList.toggle('has', shelf.length > 0);
    shelfCount.textContent = shelf.length ? shelf.length + ' item' : '';
    shelfList.textContent = '';
    for (const it of shelf) {
      const card = document.createElement('div');
      card.className = 'card' + (newPaths && newPaths.has(it.path) ? ' new' : '');
      card.title = it.path;
      card.draggable = false;
      card.addEventListener('dragstart', e => e.preventDefault());

      let downPt = null;
      card.addEventListener('pointerdown', e => {
        if (e.target.closest('.acts')) return;
        downPt = { x: e.clientX, y: e.clientY };
      });
      card.addEventListener('pointermove', e => {
        if (!downPt || e.buttons !== 1) return;
        const dx = e.clientX - downPt.x;
        const dy = e.clientY - downPt.y;
        if (dx * dx + dy * dy > 36) {
          downPt = null;
          draggingOut = true;
          invoke('shelf_drag_out', { paths: [it.path] }).catch(err => {
            draggingOut = false;
            toastShelf('gagal: ' + err);
          });
        }
      });
      card.addEventListener('pointerup', () => {
        downPt = null;
      });

      card.addEventListener('click', e => {
        if (e.target.closest('.acts')) return;
        invoke('open_path', { path: it.path }).catch(() => {
          card.classList.add('missing');
          toastShelf('file tidak ditemukan');
        });
      });

      const ico = document.createElement('div');
      ico.className = 'ico';
      if (it.dir) ico.innerHTML = '<svg viewBox="0 0 24 24">' + FOLDER_SVG + '</svg>';
      else if (/^(jpe?g|png|gif|webp|bmp|svg)$/.test(it.ext || ''))
        ico.innerHTML = '<svg viewBox="0 0 24 24">' + IMAGE_SVG + '</svg>';
      else ico.textContent = (it.ext || 'file').slice(0, 3);

      const nm = document.createElement('span');
      nm.className = 'nm';
      nm.textContent = it.name || it.path;

      const sz = document.createElement('span');
      sz.className = 'sz';
      sz.textContent = it.dir ? 'folder' : fmtSize(it.size);

      const acts = document.createElement('div');
      acts.className = 'acts';

      const copyBtn = document.createElement('button');
      copyBtn.className = 'copy';
      copyBtn.title = 'Salin';
      copyBtn.innerHTML =
        '<svg viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
      copyBtn.addEventListener('click', ev => {
        ev.stopPropagation();
        invoke('shelf_copy', { paths: [it.path] })
          .then(() => toastShelf('disalin'))
          .catch(() => toastShelf('gagal'));
      });

      const revBtn = document.createElement('button');
      revBtn.className = 'reveal';
      revBtn.title = 'Buka lokasi';
      revBtn.innerHTML = '<svg viewBox="0 0 24 24">' + FOLDER_SVG + '</svg>';
      revBtn.addEventListener('click', ev => {
        ev.stopPropagation();
        invoke('reveal_path', { path: it.path }).catch(() => toastShelf('gagal'));
      });

      const x = document.createElement('button');
      x.className = 'x';
      x.textContent = '×';
      x.title = 'Hapus';
      x.addEventListener('click', ev => {
        ev.stopPropagation();
        shelf = shelf.filter(s => s.path !== it.path);
        saveShelf();
        renderShelf();
      });

      acts.append(copyBtn, revBtn, x);
      card.append(ico, nm, sz, acts);
      shelfList.appendChild(card);
    }
  }

  shelfClear.addEventListener('click', () => {
    shelf = [];
    saveShelf();
    renderShelf();
  });

  shelfPaste.addEventListener('click', () => {
    invoke('shelf_from_clipboard')
      .then(info => addShelf(info, true))
      .catch(() => toastShelf('clipboard tidak berisi file'));
  });

  listen('shelf:drag-end', () => {
    draggingOut = false;
    if (!hover) leaveTimer = setTimeout(maybeClose, 300);
  });

  listen('drag-near', e => {
    dragging = !!e.payload;
    if (dragging && !draggingOut) {
      openNotch('shelf');
      body.classList.add('dropping');
    } else if (!dragging) {
      body.classList.remove('dropping');
      if (!hover) leaveTimer = setTimeout(maybeClose, 600);
    }
  });

  listen('tauri://drag-enter', () => {
    if (draggingOut) return;
    if (state !== 'open' || page !== 'shelf') openNotch('shelf');
    body.classList.add('dropping');
  });

  listen('tauri://drag-leave', () => {
    if (!dragging) body.classList.remove('dropping');
  });

  listen('tauri://drag-drop', e => {
    body.classList.remove('dropping');
    if (draggingOut) return;
    const paths = (e.payload && e.payload.paths) || [];
    if (!paths.length) return;
    invoke('shelf_info', { paths })
      .then(info => addShelf(info, true))
      .catch(err => alertState('gagal: ' + err, 3000));
  });

  // Native OLE catcher: the only drop path that actually receives Explorer files.
  listen('shelf:drop', e => {
    body.classList.remove('dropping');
    const paths = e.payload || [];
    if (!paths.length || draggingOut) return;
    invoke('shelf_info', { paths })
      .then(info => addShelf(info, true))
      .catch(err => toastShelf('gagal: ' + err));
  });

  // ---------- system HUD ----------
  function showHud(p) {
    if (!['idle', 'live', 'hud'].includes(state)) return;
    let icon = 'speaker';
    let label = '';
    let val = '';
    let bar = null;
    let nobar = false;
    if (p.kind === 'volume') {
      icon = p.muted ? 'muted' : 'speaker';
      label = p.muted ? 'Bisu' : 'Volume';
      val = (p.value == null ? 0 : p.value) + '%';
      bar = p.value;
    } else if (p.kind === 'caps') {
      icon = 'caps';
      label = p.on ? 'Caps Lock nyala' : 'Caps Lock mati';
      val = p.on ? 'ON' : 'OFF';
      nobar = true;
    } else if (p.kind === 'power') {
      icon = p.battery != null ? 'battery' : 'power';
      label = p.plugged
        ? 'Mengisi daya'
        : p.battery != null && p.battery <= 20
        ? 'Baterai lemah'
        : 'Charger dicabut';
      val = p.battery != null ? p.battery + '%' : '';
      bar = p.battery;
      nobar = p.battery == null;
      if (p.plugged) icon = 'power';
    } else if (p.kind === 'lang') {
      icon = 'keyboard';
      label = 'Bahasa keyboard';
      val = String(p.value || '').toUpperCase();
      nobar = true;
    } else if (p.kind === 'net') {
      icon = 'wifi';
      label = p.online ? 'Internet tersambung' : 'Internet terputus';
      val = '';
      nobar = true;
    } else if (p.kind === 'audio') {
      icon = /headphone|headset|buds|airpods|earbud/i.test(p.name || '')
        ? 'headphone'
        : 'speaker';
      label = p.name || 'Perangkat audio';
      val = 'Output';
      nobar = true;
    }
    hudIcon.innerHTML = HUD_ICONS[icon] || '';
    hudLabel.textContent = label;
    hudVal.textContent = val;
    hudEl.classList.toggle('nobar', nobar);
    hudEl.classList.toggle('charge', p.kind === 'power' && !!p.plugged);
    hudEl.classList.toggle('low', p.kind === 'power' && !p.plugged && p.battery != null && p.battery <= 20);
    hudBar.style.width = (bar == null ? 0 : Math.max(0, Math.min(100, bar))) + '%';
    go('hud');
    clearTimeout(hudTimer);
    hudTimer = setTimeout(() => {
      if (state === 'hud') go(topActivity() ? 'live' : 'idle');
    }, 1600);
  }

  listen('hud', e => showHud(e.payload));

  function setBatt(battery, plugged) {
    if (battery == null) {
      battEl.style.display = 'none';
      battEl.textContent = '';
      return;
    }
    battEl.style.display = 'inline';
    battEl.textContent = battery + '%';
    battEl.classList.toggle('plug', !!plugged);
    battEl.title = plugged ? 'mengisi daya' : 'baterai';
  }

  listen('sys', e => setBatt(e.payload.battery, e.payload.plugged));

  // ---------- agent ----------
  listen('agent', e => {
    const a = e.payload;
    if (a.state === 'done') {
      activities.agent = null;
      alertState((a.agent || 'agent') + ' selesai', 5000);
    } else if (a.state === 'waiting') {
      activities.agent = a;
      alertState((a.agent || 'agent') + ': ' + (a.text || 'menunggu'), 6000);
    } else {
      activities.agent = a;
      applyActivityLabel();
      updateActivityRow();
    }
  });

  // ---------- navigation ----------
  $('#prevPage').addEventListener('click', () => stepPage(-1));
  $('#nextPage').addEventListener('click', () => stepPage(1));

  island.addEventListener(
    'wheel',
    ev => {
      if (state !== 'open') return;
      const sc = ev.target.closest && ev.target.closest('.scroll');
      if (sc && sc.scrollHeight > sc.clientHeight + 1) return;
      const now = Date.now();
      if (now - wheelAt < 320) return;
      wheelAt = now;
      stepPage(ev.deltaY > 0 ? 1 : -1);
    },
    { passive: true }
  );

  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') hideAll();
    if (
      (e.ctrlKey || e.metaKey) &&
      e.key.toLowerCase() === 'v' &&
      state === 'open' &&
      page === 'shelf'
    ) {
      invoke('shelf_from_clipboard')
        .then(info => addShelf(info, true))
        .catch(() => toastShelf('clipboard tidak berisi file'));
    }
  });

  input.addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send(input.value);
    }
  });

  input.addEventListener('pointerdown', () => invoke('focus_input').catch(() => {}));

  input.addEventListener('input', () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 84) + 'px';
  });

  $('#send').addEventListener('click', () => send(input.value));
  $('#mic').addEventListener('click', startListening);

  ttsBtn.addEventListener('click', () => {
    ttsOn = !ttsOn;
    ttsBtn.textContent = 'suara: ' + (ttsOn ? 'on' : 'off');
  });

  // ---------- clock ----------
  function tickClock() {
    const d = new Date();
    const hm = String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
    clockEl.textContent = hm;
    icTime.textContent = hm;
    icDate.textContent = d.toLocaleDateString('id-ID', {
      weekday: 'short',
      day: 'numeric',
      month: 'short',
    });
  }
  tickClock();
  setInterval(tickClock, 15000);

  // ---------- blink ----------
  (function blink() {
    setTimeout(() => {
      if (!body.classList.contains('sleepy')) {
        for (const eye of mascot.querySelectorAll('.eye')) {
          eye.classList.add('blink');
          setTimeout(() => eye.classList.remove('blink'), 140);
        }
      }
      blink();
    }, 3000 + Math.random() * 3000);
  })();

  // ---------- boot ----------
  body.dataset.page = page;
  markPage();
  applySize();
  renderDots();
  renderMusic();
  renderFocus();
  renderTodos();
  renderWater();
  renderShelf();
  armSleep();

  invoke('info')
    .then(i => {
      modelEl.textContent = i.model + ' | stt: ' + i.stt;
      ttsOn = !!i.tts;
      ttsBtn.textContent = 'suara: ' + (ttsOn ? 'on' : 'off');
      autoCollapseMs = i.autoCollapseMs || 5000;
      applySize();
    })
    .catch(() => {});

  invoke('sys_status')
    .then(s => setBatt(s.battery, s.plugged))
    .catch(() => {});

  function renderPage() {
    if (page === 'music') {
      renderMusic();
      fetchLyrics();
    } else if (page === 'focus') renderFocus();
    else if (page === 'todo') renderTodos();
    else if (page === 'water') renderWater();
    else if (page === 'shelf') renderShelf();
    else if (page === 'chat' && !pending) renderHistory();
  }
})();
