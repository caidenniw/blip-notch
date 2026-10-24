'use strict';

// Pure helpers shared by the WebView UI and node tests. No DOM access here.

function parseTimer(text) {
  const m = /\b(?:timer|pasang timer|ingatkan(?: saya)?(?: dalam)?)\s*(?:selama\s*)?(\d+)\s*(detik|menit|jam)\b/i.exec(text || '');
  if (!m) return null;
  const n = parseInt(m[1], 10);
  const unit = m[2].toLowerCase();
  const mult = unit === 'detik' ? 1 : unit === 'menit' ? 60 : 3600;
  const secs = n * mult;
  return secs > 0 ? secs : null;
}

function fmt(secs) {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return String(m).padStart(2, '0') + ':' + String(s).padStart(2, '0');
}

function fmtTrack(secs) {
  const t = Math.floor(Math.max(0, secs));
  return Math.floor(t / 60) + ':' + String(t % 60).padStart(2, '0');
}

function trackPos(m, now) {
  if (!m || !m.duration) return 0;
  const p = m.position + (m.playing ? (now - m.at) / 1000 : 0);
  return Math.max(0, Math.min(m.duration, p));
}

const APPS = {
  spotify: 'spotify:', youtube: 'https://www.youtube.com', 'youtube music': 'https://music.youtube.com',
  whatsapp: 'whatsapp:', gmail: 'https://mail.google.com', github: 'https://github.com', chatgpt: 'https://chatgpt.com',
};

// Local commands handled without the LLM. Short phrases only, so real questions still reach the model.
function parseIntent(text) {
  const t = String(text || '').toLowerCase().trim().replace(/[.,!?]+$/, '').trim();
  if (!t) return null;

  let m = /^(?:tambah|catat)\s+(?:tugas|todo)\s+(.+)$/.exec(t);
  if (m) {
    const body = m[1].trim();
    if (!body || body.split(/\s+/).length > 20) return null;
    return { type: 'todo', text: body };
  }

  const words = t.split(/\s+/).length;
  if (words > 7) return null;

  if (/^(?:sudah\s+)?minum(?:\s+air)?$/.test(t)) return { type: 'water' };
  if (/^(?:mulai\s+)?(?:fokus|pomodoro)$/.test(t)) return { type: 'focus', cmd: 'start' };
  if (/^(?:stop|berhenti|hentikan|selesai)\s+(?:fokus|pomodoro)$/.test(t)) return { type: 'focus', cmd: 'stop' };

  m = /^(?:putar|puter|mainkan|play)\s+(.+?)\s+di\s+(youtube|spotify)$/.exec(t);
  if (m) {
    const q = encodeURIComponent(m[1]);
    return m[2] === 'spotify'
      ? { type: 'open', label: 'spotify', target: 'spotify:search:' + q }
      : { type: 'open', label: 'youtube', target: 'https://www.youtube.com/results?search_query=' + q };
  }
  m = /^(?:buka|open|jalankan)\s+(.+)$/.exec(t);
  if (m && APPS[m[1]]) return { type: 'open', label: m[1], target: APPS[m[1]] };

  if (/\b(volume|suara)\b/.test(t)) {
    m = /\b(?:volume|suara)\s+(\d{1,3})\b/.exec(t);
    if (m) return { type: 'volume', dir: 'set', level: Math.min(100, parseInt(m[1], 10)) };
    const steps = /\b(banget|sekali|full)\b/.test(t) ? 10 : 5;
    if (/\b(mute|bisu|matikan)\b/.test(t)) return { type: 'volume', dir: 'mute', steps: 1 };
    if (/\b(naik|besar|besarkan|keras|kencang|tambah|up)\b/.test(t)) return { type: 'volume', dir: 'up', steps };
    if (/\b(turun|kecil|kecilkan|pelan|kurangi|down)\b/.test(t)) return { type: 'volume', dir: 'down', steps };
  }

  if (/\b(lagu|lagunya|musik|musiknya|music|track)\b/.test(t) || words <= 2) {
    if (/\b(next|skip|selanjutnya|berikutnya)\b|ganti lagu/.test(t)) return { type: 'media', cmd: 'next' };
    if (/\b(sebelumnya|previous|prev)\b|lagu tadi/.test(t)) return { type: 'media', cmd: 'prev' };
    if (/\b(pause|jeda|stop|berhenti|hentikan)\b/.test(t)) return { type: 'media', cmd: 'pause' };
    if (/\b(play|putar|puter|lanjut|lanjutkan|resume)\b/.test(t)) return { type: 'media', cmd: 'play' };
  }
  return null;
}

const INTENT_LABEL = {
  next: 'lagu berikutnya', prev: 'lagu sebelumnya', pause: 'musik dijeda', play: 'musik diputar',
  up: 'volume naik', down: 'volume turun', mute: 'volume mute',
  water: '+250 ml dicatat', focusStart: 'fokus dimulai', focusStop: 'fokus dihentikan',
  todo: 'tugas ditambah', set: 'volume diatur',
};

function normKey(s) {
  return String(s || '').toLowerCase().normalize('NFKD').replace(/[^a-z0-9]/g, '');
}

function parseTrack(title, artist) {
  let a = String(artist || '').trim();
  a = a.replace(/\s*-\s*topic$/i, '').replace(/vevo$/i, '').replace(/\s*official( channel)?$/i, '').trim();
  let t = String(title || '').split(' | ')[0];
  t = t.replace(/\([^)]*\)/g, ' ').replace(/\[[^\]]*\]/g, ' ');
  t = t.replace(/\s+-\s+(remaster(ed)?( \d{4})?|live|radio edit|single version)\b.*$/i, '');
  t = t.replace(/\b(official\s+(music\s+)?video|official\s+audio|lyrics?\s+video|visuali[sz]er|mv|hd|4k)\b/gi, '');
  t = t.replace(/\s+/g, ' ').replace(/-+$/, '').trim();
  if (t.includes(' - ')) {
    const idx = t.indexOf(' - ');
    const left = t.slice(0, idx).trim();
    const right = t.slice(idx + 3).trim();
    const nl = normKey(left);
    const na = normKey(a);
    if (!a || (nl && na && (nl.includes(na) || na.includes(nl)))) {
      return { track: right, artist: left };
    }
  }
  return { track: t, artist: a };
}

function appName(id) {
  const s = String(id || '').trim();
  if (!s) return '';
  const low = s.toLowerCase();
  if (low.includes('spotify')) return 'Spotify';
  if (low.includes('brave')) return 'Brave';
  if (low.includes('chrome')) return 'Chrome';
  if (low.includes('msedge') || low.includes('edge')) return 'Edge';
  if (low.includes('firefox')) return 'Firefox';
  if (low.includes('zune')) return 'Media Player';
  if (low.includes('applemusic') || low.includes('itunes')) return 'Apple Music';
  const parts = s.split(/[!\\]/);
  const tail = parts[parts.length - 1];
  const cleaned = tail.replace(/\.exe$/i, '').trim();
  return cleaned.slice(0, 16);
}

// LRC lines may carry several [mm:ss.xx] tags; repeated tags become repeated rows.
function parseLrc(text) {
  const out = [];
  const off = /^\[offset:\s*([+-]?\d+)\s*\]/im.exec(String(text || ''));
  const shift = off ? Number(off[1]) / 1000 : 0;
  for (const raw of String(text || '').split(/\r?\n/)) {
    const tags = [...raw.matchAll(/\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g)];
    if (!tags.length) continue;
    const line = raw.replace(/\[[^\]]*\]/g, '').trim();
    if (!line) continue;
    for (const m of tags) {
      const frac = m[3] ? Number((m[3] + '00').slice(0, 3)) / 1000 : 0;
      const t = Number(m[1]) * 60 + Number(m[2]) + frac;
      out.push({ t: Math.max(0, t - shift), line });
    }
  }
  out.sort((a, b) => a.t - b.t);
  return out;
}

function lrcIndex(lines, pos) {
  let idx = -1;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].t <= pos + 0.25) idx = i;
    else break;
  }
  return idx;
}

// Trailing "@14:30", "jam 14:30" or "jam 14" becomes a due time today, or tomorrow when already past.
function parseTodo(text, now) {
  const base = now == null ? Date.now() : now;
  let t = String(text || '').trim();
  const m = /\s+(?:@\s*(\d{1,2})(?::(\d{2}))?|jam\s+(\d{1,2})(?::(\d{2}))?)\s*$/i.exec(t);
  if (!m) return { text: t, dueAt: null };
  const hh = Number(m[1] != null ? m[1] : m[3]);
  const mm = Number(m[2] != null ? m[2] : m[4] != null ? m[4] : 0);
  t = t.slice(0, m.index).trim();
  if (hh > 23 || mm > 59) return { text: t, dueAt: null };
  const d = new Date(base);
  d.setHours(hh, mm, 0, 0);
  if (d.getTime() <= base) d.setDate(d.getDate() + 1);
  return { text: t, dueAt: d.getTime() };
}

function dayKey(ms) {
  const d = new Date(ms);
  return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
}

function waterBuckets(entries, startHour, endHour) {
  const n = Math.max(0, Math.floor((endHour - startHour) / 2));
  const out = Array(n).fill(0);
  for (const e of entries || []) {
    const d = new Date(e.t);
    const i = Math.floor((d.getHours() + d.getMinutes() / 60 - startHour) / 2);
    if (i >= 0 && i < n) out[i] += Number(e.ml) || 0;
  }
  return out;
}

function fmtSize(n) {
  n = Math.max(0, Number(n) || 0);
  if (n < 1024) return Math.round(n) + ' B';
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return (v >= 100 ? v.toFixed(0) : v.toFixed(1)) + ' ' + units[i];
}

function fileKind(ext, dir) {
  if (dir) return 'folder';
  const value = String(ext || '').toLowerCase().replace(/^\./, '');
  if (/^(jpe?g|png|gif|webp|bmp|svg|heic)$/.test(value)) return 'image';
  if (value === 'pdf') return 'pdf';
  if (/^(doc|docx|odt|rtf)$/.test(value)) return 'word';
  if (/^(xls|xlsx|csv|ods)$/.test(value)) return 'sheet';
  if (/^(ppt|pptx|odp)$/.test(value)) return 'slides';
  if (/^(zip|rar|7z|tar|gz)$/.test(value)) return 'archive';
  if (/^(mp3|wav|flac|m4a|ogg)$/.test(value)) return 'audio';
  if (/^(mp4|mkv|avi|mov|webm)$/.test(value)) return 'video';
  if (/^(js|ts|tsx|jsx|py|rs|json|html|css|md|txt)$/.test(value)) return 'code';
  return 'file';
}

// RGBA pixels → mean [r,g,b], skipping translucent, near-black and near-white pixels; null if none.
function avgColor(data) {
  let r = 0;
  let g = 0;
  let b = 0;
  let n = 0;
  for (let i = 0; i + 3 < data.length; i += 4) {
    if (data[i + 3] < 128) continue;
    const pr = data[i];
    const pg = data[i + 1];
    const pb = data[i + 2];
    const max = Math.max(pr, pg, pb);
    const min = Math.min(pr, pg, pb);
    if (max < 24 || min > 232) continue;
    r += pr;
    g += pg;
    b += pb;
    n++;
  }
  if (!n) return null;
  return [Math.round(r / n), Math.round(g / n), Math.round(b / n)];
}

const CHAT_MIN = 170, CHAT_MAX = 280;
function chatHeightFor(chrome, content) {
  return Math.max(CHAT_MIN, Math.min(CHAT_MAX, Math.ceil((chrome + content) / 8) * 8));
}
function level01(rms, gain) {
  const v = Number(rms) * gain;
  return v > 0 ? Math.min(1, v) : 0; // NaN and negatives -> 0
}

function trimHistory(h, n) {
  const out = h.slice(-n);
  while (out.length && out[0].role !== 'user') out.shift();
  return out;
}

function parseMemory(text) {
  let m = /^\s*(?:tolong\s+)?ingat(?:lah)?\s+(?:bahwa\s+)?(.{3,200}?)\s*$/i.exec(text);
  if (m) return m[1].endsWith('?') ? null : { type: 'remember', text: m[1] };
  if (/^\s*lupakan\s+(?:semua|semuanya)\s*$/i.test(text)) return { type: 'forget' };
  if (/^\s*(?:apa\s+(?:yang\s+)?kamu\s+ingat|kamu\s+ingat\s+apa)\s*\??\s*$/i.test(text)) return { type: 'recall' };
  return null;
}

function greeting(hour) {
  return hour < 11 ? 'Selamat pagi' : hour < 15 ? 'Selamat siang' : hour < 18 ? 'Selamat sore' : 'Selamat malam';
}

function nextTask(todos) {
  const open = todos.filter(t => !t.done);
  const due = open.filter(t => t.dueAt);
  if (due.length) return due.reduce((a, b) => (b.dueAt < a.dueAt ? b : a));
  return open[0] || null;
}

// First matching proactive nudge, or null. `c.shown` maps nudge id -> last shown ms (per-id cooldown).
function pickNudge(c) {
  const ok = (id, ms) => !c.shown[id] || c.now - c.shown[id] >= ms;
  if (c.battery != null && c.battery <= 20 && !c.plugged && ok('battery', 30 * 60000)) {
    return { id: 'battery', text: `baterai ${c.battery}%, colok charger`, tone: '', mood: 'scared' };
  }
  if ((c.hour >= 23 || c.hour < 4) && ok('night', 8 * 3600000)) {
    return { id: 'night', text: 'sudah larut, jangan lupa istirahat', tone: 'ok', mood: 'tired' };
  }
  if (c.hour >= 17 && c.hour < 19 && c.tasksLeft > 0 && ok('evening', 12 * 3600000)) {
    return { id: 'evening', text: `${c.tasksLeft} tugas belum selesai hari ini`, tone: 'ok', mood: 'happy' };
  }
  if (c.hour >= 5 && c.hour < 22 && ok('greet', 12 * 3600000)) {
    return {
      id: 'greet',
      text: `${greeting(c.hour)}. ${c.tasksLeft ? c.tasksLeft + ' tugas menunggu' : 'belum ada tugas'}`,
      tone: 'ok',
      mood: 'cheer',
    };
  }
  return null;
}

if (typeof module !== 'undefined' && module.exports) {
  module.exports = {
    parseTimer, fmt, fmtTrack, trackPos, APPS, parseIntent, INTENT_LABEL,
    normKey, parseTrack, appName, parseLrc, lrcIndex, parseTodo, dayKey, waterBuckets, fmtSize,
    avgColor, fileKind, chatHeightFor, level01,
    trimHistory, parseMemory, greeting, nextTask, pickNudge,
  };
}
