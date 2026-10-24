'use strict';
const { strictEqual, deepStrictEqual } = require('assert');
const {
  parseTimer, parseIntent, fmt, fmtTrack, trackPos,
  normKey, parseTrack, appName, parseLrc, lrcIndex, parseTodo, dayKey, waterBuckets, fmtSize,
  avgColor, fileKind, chatHeightFor, level01,
  trimHistory, parseMemory, greeting, nextTask, pickNudge,
} = require('./lib.js');

strictEqual(parseTimer('timer 5 menit'), 300);
strictEqual(fmt(65), '01:05');
strictEqual(fmtTrack(65.9), '1:05');
strictEqual(fmtTrack(-3), '0:00');
strictEqual(trackPos({ position: 10, duration: 100, playing: true, at: 0 }, 5000), 15);
strictEqual(trackPos({ position: 98, duration: 100, playing: true, at: 0 }, 10000), 100);
strictEqual(trackPos({ position: 10, duration: 100, playing: false, at: 0 }, 5000), 10);
strictEqual(trackPos({ position: 10, duration: 0, playing: true, at: 0 }, 5000), 0);

deepStrictEqual(parseIntent('lagu berikutnya'), { type: 'media', cmd: 'next' });
deepStrictEqual(parseIntent('skip'), { type: 'media', cmd: 'next' });
deepStrictEqual(parseIntent('pause'), { type: 'media', cmd: 'pause' });
deepStrictEqual(parseIntent('lanjutkan musik'), { type: 'media', cmd: 'play' });
deepStrictEqual(parseIntent('lagu sebelumnya'), { type: 'media', cmd: 'prev' });
strictEqual(parseIntent('apa langkah berikutnya'), null);
deepStrictEqual(parseIntent('volume naik'), { type: 'volume', dir: 'up', steps: 5 });
deepStrictEqual(parseIntent('kecilkan suara banget'), { type: 'volume', dir: 'down', steps: 10 });
deepStrictEqual(parseIntent('mute suara'), { type: 'volume', dir: 'mute', steps: 1 });
deepStrictEqual(parseIntent('buka spotify'), { type: 'open', label: 'spotify', target: 'spotify:' });
strictEqual(parseIntent('buka notepad'), null);
strictEqual(parseIntent('putar lofi hip hop di youtube').target, 'https://www.youtube.com/results?search_query=lofi%20hip%20hop');
strictEqual(parseIntent('Putar Hindia di Spotify!').target, 'spotify:search:hindia');

// v3 intents
deepStrictEqual(parseIntent('volume 50'), { type: 'volume', dir: 'set', level: 50 });
deepStrictEqual(parseIntent('volume 250'), { type: 'volume', dir: 'set', level: 100 });
deepStrictEqual(parseIntent('tambah tugas kirim laporan jam 14'), { type: 'todo', text: 'kirim laporan jam 14' });
deepStrictEqual(parseIntent('minum'), { type: 'water' });
deepStrictEqual(parseIntent('sudah minum air'), { type: 'water' });
deepStrictEqual(parseIntent('mulai fokus'), { type: 'focus', cmd: 'start' });
deepStrictEqual(parseIntent('pomodoro'), { type: 'focus', cmd: 'start' });
deepStrictEqual(parseIntent('stop fokus'), { type: 'focus', cmd: 'stop' });

// lyrics & tracks
deepStrictEqual(
  parseTrack('Justin Timberlake - Mirrors (Official Video)', 'justintimberlakeVEVO'),
  { track: 'Mirrors', artist: 'Justin Timberlake' }
);
deepStrictEqual(
  parseTrack('Tems - Me & U (Official Video)', 'TemsVEVO'),
  { track: 'Me & U', artist: 'Tems' }
);
deepStrictEqual(
  parseTrack('Doubt', 'Twenty One Pilots - Topic'),
  { track: 'Doubt', artist: 'Twenty One Pilots' }
);
deepStrictEqual(
  parseTrack('Mirrors', 'Justin Timberlake'),
  { track: 'Mirrors', artist: 'Justin Timberlake' }
);
deepStrictEqual(
  parseTrack('Song [Official Music Video] | HD', 'Band'),
  { track: 'Song', artist: 'Band' }
);
deepStrictEqual(
  parseTrack('Hey Jude - Remastered 2015', 'The Beatles'),
  { track: 'Hey Jude', artist: 'The Beatles' }
);
strictEqual(appName('Spotify.exe'), 'Spotify');
strictEqual(appName(''), '');
const lrc = parseLrc('[00:23.76]Scared\n[00:25.76]of\n');
deepStrictEqual(lrc, [{ t: 23.76, line: 'Scared' }, { t: 25.76, line: 'of' }]);
strictEqual(lrcIndex(lrc, 24), 0);
strictEqual(lrcIndex(lrc, 10), -1);
strictEqual(lrcIndex(lrc, 99), 1);
deepStrictEqual(parseLrc('[00:10.00][00:20.00]chorus'), [{ t: 10, line: 'chorus' }, { t: 20, line: 'chorus' }]);
deepStrictEqual(parseLrc('[offset:500]\n[00:10.00]a'), [{ t: 9.5, line: 'a' }]);

// todo
const morning = new Date(2026, 9, 19, 9, 0).getTime();
const afternoon = new Date(2026, 9, 19, 15, 0).getTime();
deepStrictEqual(parseTodo('kirim laporan @14:30', morning), {
  text: 'kirim laporan', dueAt: new Date(2026, 9, 19, 14, 30).getTime(),
});
deepStrictEqual(parseTodo('kirim laporan @14:30', afternoon), {
  text: 'kirim laporan', dueAt: new Date(2026, 9, 20, 14, 30).getTime(),
});
strictEqual(parseTodo('beli susu', morning).dueAt, null);
strictEqual(parseTodo('rapat jam 14', morning).dueAt, new Date(2026, 9, 19, 14, 0).getTime());

// water
strictEqual(dayKey(new Date(2026, 9, 19, 9, 10).getTime()), '2026-10-19');
const buckets = waterBuckets([{ t: new Date(2026, 9, 19, 9, 10).getTime(), ml: 250 }], 8, 22);
strictEqual(buckets.length, 7);
strictEqual(buckets[0], 250);
strictEqual(buckets[1], 0);

// sizes
strictEqual(fmtSize(512), '512 B');
strictEqual(fmtSize(2516582), '2.4 MB');
strictEqual(fmtSize(0), '0 B');

// cover tint
deepStrictEqual(avgColor([255, 0, 0, 255, 0, 0, 255, 255]), [128, 0, 128]);
strictEqual(avgColor([0, 0, 0, 255, 255, 255, 255, 255]), null);
strictEqual(avgColor([10, 200, 10, 0]), null);

// Shelf file classification
strictEqual(fileKind('', true), 'folder');
strictEqual(fileKind('PDF'), 'pdf');
strictEqual(fileKind('docx'), 'word');
strictEqual(fileKind('xlsx'), 'sheet');
strictEqual(fileKind('pptx'), 'slides');
strictEqual(fileKind('zip'), 'archive');
strictEqual(fileKind('mp3'), 'audio');
strictEqual(fileKind('mp4'), 'video');
strictEqual(fileKind('ts'), 'code');
strictEqual(fileKind('jpg'), 'image');
strictEqual(fileKind('bin'), 'file');
strictEqual(chatHeightFor(142, 0), 170);
strictEqual(chatHeightFor(142, 60), 208);
strictEqual(chatHeightFor(142, 500), 280);
strictEqual(level01(0.1, 5), 0.5);
strictEqual(level01(0.5, 5), 1);
strictEqual(level01(-1, 5), 0);
strictEqual(level01(NaN, 5), 0);

const H = [{ role: 'assistant', content: 'a' }, { role: 'user', content: 'b' }, { role: 'assistant', content: 'c' }];
deepStrictEqual(trimHistory(H, 2), [H[1], H[2]]);
deepStrictEqual(trimHistory(H, 1), []);

deepStrictEqual(parseMemory('ingat nama saya Caiden'), { type: 'remember', text: 'nama saya Caiden' });
strictEqual(parseMemory('ingat bahwa saya suka kopi').text, 'saya suka kopi');
strictEqual(parseMemory('kamu ingat tidak kemarin?'), null);
strictEqual(parseMemory('ingat apa kemarin?'), null);
deepStrictEqual(parseMemory('lupakan semua'), { type: 'forget' });
deepStrictEqual(parseMemory('apa yang kamu ingat?'), { type: 'recall' });

deepStrictEqual([5, 12, 16, 20].map(greeting), ['Selamat pagi', 'Selamat siang', 'Selamat sore', 'Selamat malam']);

const T = [{ text: 'x', done: false }, { text: 'late', done: false, dueAt: 200 }, { text: 'soon', done: false, dueAt: 100 }];
strictEqual(nextTask(T).text, 'soon');
strictEqual(nextTask([T[0], { text: 'y', done: false }]).text, 'x');
strictEqual(nextTask([{ text: 'd', done: true }]), null);

const N = 1e12;
const base = { now: N, hour: 12, tasksLeft: 2, battery: 15, plugged: false, shown: {} };
strictEqual(pickNudge(base).id, 'battery');
strictEqual(pickNudge({ ...base, shown: { battery: N - 60000 } }).id, 'greet');
strictEqual(pickNudge({ ...base, plugged: true }).id, 'greet');
strictEqual(pickNudge({ ...base, battery: null, hour: 23 }).id, 'night');
strictEqual(pickNudge({ ...base, battery: null, hour: 17, shown: { greet: N - 1000 } }).id, 'evening');
strictEqual(pickNudge({ ...base, battery: null, hour: 3, shown: { night: N - 1000 } }), null);


console.log('ok');
