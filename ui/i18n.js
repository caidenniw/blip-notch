'use strict';

// UI strings. `en` is the default; `id` keeps the original Indonesian text. Key sets must match.
const STR = {
  en: {
    'page.today': 'Today', 'page.music': 'Music', 'page.focus': 'Focus', 'page.todo': 'Tasks',
    'page.water': 'Water', 'page.shelf': 'Files', 'page.chat': 'Chat', 'page.nav': 'Blip pages',
    'hdr.settings': 'Settings', 'hdr.pin': 'Pin',

    'music.seek': 'Seek', 'music.lyrics': 'Lyrics', 'music.prev': 'Previous', 'music.toggle': 'Play/Pause',
    'music.next': 'Next', 'music.lyrSlower': 'Lyrics later', 'music.lyrFaster': 'Lyrics earlier',
    'music.lyrReset': 'Double-click to reset', 'music.untitled': 'music',
    'lyr.loading': 'searching lyrics…', 'lyr.none': 'lyrics not found', 'lyr.plain': 'lyrics without timing (not synced)',
    'lyr.error': 'lyrics failed to load · click the icon to retry',

    'focus.phase.focus': 'Focus', 'focus.phase.short': 'Break', 'focus.phase.long': 'Long break',
    'focus.chip.focus': 'Focus', 'focus.chip.short': 'Break', 'focus.chip.long': 'Long',
    'focus.session': 'session {n}/4', 'focus.minus': 'Minus 5 minutes', 'focus.plus': 'Plus 5 minutes',
    'focus.minutes': 'Minutes', 'focus.min': 'min', 'focus.start': 'Start', 'focus.pause': 'Pause',
    'focus.resume': 'Resume', 'focus.reset': 'Reset', 'focus.skip': 'Skip',
    'focus.timeBreak': 'time for a break', 'focus.timeFocus': 'time to focus',

    'todo.count': '{done} done today · {left} left', 'todo.clear': 'Clear done', 'todo.placeholder': 'New task…',
    'todo.add': 'Add', 'todo.empty': 'Nothing here yet. What would you like to get done?',
    'todo.remind': 'Remind me', 'todo.removeReminder': 'Remove reminder', 'todo.in15': 'In 15 minutes', 'todo.in60': 'In 1 hour',
    'todo.chooseTime': 'Choose date & time', 'todo.day': 'Day', 'todo.month': 'Month', 'todo.year': 'Year', 'todo.hour': 'Hour', 'todo.minute': 'Min',
    'todo.setReminder': 'Set', 'todo.invalidTime': 'Choose a valid date and a future time.', 'todo.todayAt': 'Today, {t}', 'todo.tomorrowAt': 'Tomorrow, {t}',
    'todo.markOpen': 'Mark not done', 'todo.markDone': 'Mark done', 'todo.editHint': '{text} (double-click to edit)',
    'todo.delete': 'Delete', 'todo.tomorrow': 'tomorrow {t}', 'todo.allDone': 'all done',

    'water.undo': 'Undo', 'water.goalLbl': 'goal', 'water.cupLbl': 'cup', 'water.everyLbl': 'every',
    'water.goalReached': 'Goal reached', 'water.pctToday': '{n}% today', 'water.goalAlert': 'water goal reached',
    'water.remind': 'time to drink water',
    'today.focusSessions': '{n} sessions today',

    'shelf.paste': 'Paste', 'shelf.clear': 'Clear', 'shelf.dropTitle': 'Drag files here',
    'shelf.dropHint': 'Paste files from the clipboard, or drag them here', 'shelf.items': '{n} items',
    'shelf.pasteTo': 'Paste into active app', 'shelf.copied': 'copied — Ctrl+V to paste', 'shelf.open': 'Open file',
    'shelf.more': 'File options', 'shelf.reveal': 'Show in folder', 'shelf.remove': 'Remove from Shelf',
    'shelf.notFound': 'file not found', 'shelf.noClip': 'clipboard has no files',

    'chat.placeholder': 'Type or hold Right Ctrl…', 'chat.speak': 'Speak', 'chat.send': 'Send',
    'chat.qSum': 'summarize clipboard', 'chat.qTr': 'translate clipboard', 'chat.qDay': 'today', 'chat.clear': 'clear',
    'chat.voiceOn': 'voice: on', 'chat.voiceOff': 'voice: off', 'chat.empty': '(empty)', 'chat.thinking': 'thinking...',
    'chat.speaking': 'speaking...', 'chat.failed': 'failed', 'chat.failedMsg': 'failed: {e}', 'chat.answerFail': 'couldn’t answer',
    'chat.lblSum': 'Summarize clipboard', 'chat.lblTr': 'Translate clipboard', 'chat.lblDay': 'Summarize my day',
    'mem.ok': 'Okay, I’ll remember.', 'mem.okLbl': 'remembered', 'mem.forgot': 'I’ve forgotten everything.',
    'mem.forgotLbl': 'forgotten', 'mem.none': 'I don’t remember anything yet.', 'mem.recallLbl': 'memory',

    'voice.listenHint': 'tap when done', 'voice.think': 'thinking', 'voice.speakHint': 'tap to interrupt',
    'voice.micFail': 'mic failed', 'stt.empty': 'didn’t catch that', 'stt.fail': 'stt failed', 'dictate.on': 'dictating...',

    'act.waiting': 'waiting', 'act.working': 'working...', 'act.timer': 'timer',
    'timer.done': 'timer done', 'timer.set': 'timer {t} set', 'agent.done': '{a} done',

    'intent.next': 'next song', 'intent.prev': 'previous song', 'intent.pause': 'music paused', 'intent.play': 'music playing',
    'intent.up': 'volume up', 'intent.down': 'volume down', 'intent.mute': 'volume muted', 'intent.water': '+{n} ml logged',
    'intent.focusStart': 'focus started', 'intent.focusStop': 'focus stopped', 'intent.todo': 'task added',
    'intent.set': 'volume set', 'intent.volume': 'volume {n}%', 'intent.opening': 'opening {x}',

    'hud.mute': 'Muted', 'hud.volume': 'Volume', 'hud.capsOn': 'Caps Lock on', 'hud.capsOff': 'Caps Lock off',
    'hud.charging': 'Charging', 'hud.battLow': 'Low battery', 'hud.unplugged': 'Charger unplugged',
    'hud.kbd': 'Keyboard language', 'hud.netOn': 'Internet connected', 'hud.netOff': 'Internet disconnected',
    'hud.audio': 'Audio device', 'hud.bright': 'Brightness', 'hud.btOn': 'Connected', 'hud.btOff': 'Disconnected',
    'batt.charging': 'charging', 'batt.battery': 'battery',

    'greet.morning': 'Good morning', 'greet.noon': 'Good afternoon', 'greet.afternoon': 'Good afternoon', 'greet.evening': 'Good evening',
    'nudge.battery': 'battery {n}%, plug in the charger', 'nudge.night': 'it’s late, don’t forget to rest',
    'nudge.evening': '{n} tasks still open today', 'nudge.greet.tasks': '{greet}. {n} tasks waiting',
    'nudge.greet.none': '{greet}. no tasks yet',

    'set.title': 'Settings', 'set.nav.general': 'General', 'set.nav.ai': 'AI', 'set.nav.voice': 'Voice',
    'set.nav.notch': 'Notch', 'set.nav.pages': 'Pages', 'set.nav.focus': 'Focus & Water',
    'set.lang': 'Language', 'set.langHint': 'App interface language',
    'set.autostart': 'Start at login', 'set.autostartHint': 'Run Blip automatically',
    'set.hotkey': 'Open/close hotkey', 'set.hotkeyHint': 'Ctrl+Alt+Q always quits',
    'set.dictate': 'Dictation key (hold)', 'set.dictateHint': 'Hold to speak',
    'set.key.rctrl': 'Right Ctrl', 'set.key.ralt': 'Right Alt', 'set.off': 'Off',
    'set.cfg': 'Config file', 'set.cfgHint': 'Advanced settings (prompt, headers)', 'set.cfgOpen': 'Open config file',
    'set.stt': 'Speech recognition', 'set.sttHint': 'Groq needs an API key, Local uses whisper', 'set.local': 'Local',
    'set.groqKey': 'Groq API key', 'set.groqKeyHint': 'For Groq speech recognition',
    'set.sttLang': 'Speech language', 'set.sttLangHint': 'Language code, e.g. id or en',
    'set.tts': 'Read answers aloud', 'set.ttsHint': 'Speak AI answers',
    'set.engine': 'Voice engine', 'set.engineHint': 'ElevenLabs falls back to Edge on failure',
    'set.edgeVoice': 'Edge voice', 'set.edgeVoiceHint': 'e.g. id-ID-ArdiNeural', 'set.optional': 'Optional',
    'set.hover': 'Open on hover', 'set.hoverHint': 'Off: click the notch to open',
    'set.closeDelay': 'Delay before closing', 'set.closeDelayHint': 'After the cursor leaves',
    'set.fast': 'Fast', 'set.normal': 'Normal', 'set.relaxed': 'Relaxed', 'set.slow': 'Slow',
    'set.autoHide': 'Auto-hide answer', 'set.autoHideHint': 'How long answers stay', 'set.s3': '3 s', 'set.s5': '5 s', 'set.s8': '8 s',
    'set.fullscreen': 'Hide for fullscreen apps', 'set.fullscreenHint': 'Games, video, presentations',
    'set.compact': 'Compact when a window overlaps Blip', 'set.compactHint': 'Shrinks to a thin line; hover at the top edge to open',
    'set.nudge': 'Reminders', 'set.nudgeHint': 'Battery, water, focus',
    'set.motion': 'Motion', 'set.motionHint': 'Open/close animation', 'set.motionSystem': 'Follow Windows',
    'set.motionReduce': 'Reduce', 'set.motionFull': 'Full', 'set.speed': 'Animation speed', 'set.speedHint': 'Duration of all transitions',
    'set.pg.todayHint': 'Clock, tasks, water, focus', 'set.pg.musicHint': 'Shown only while media is playing',
    'set.pg.focusHint': 'Pomodoro', 'set.pg.todoHint': 'Task list', 'set.pg.waterHint': 'Water tracker',
    'set.pg.shelfHint': 'Off: dragging files onto the notch is ignored', 'set.pg.chatHint': 'always shown',
    'set.minutes': 'Minutes', 'set.waterGoal': 'Water goal (ml)', 'set.cup': 'Cup size (ml)', 'set.every': 'Remind every (minutes)',
    'set.hours': 'Start / end hour', 'set.hoursHint': 'Reminders only within this range',
    'set.saved': 'Saved', 'set.badNumber': 'Invalid number', 'set.badHours': 'End hour must be after start hour',

    'ai.endpoint': 'Endpoint', 'ai.endpointHint': 'OpenAI-compatible API', 'ai.key': 'API key', 'ai.keyHint': 'Stored in blip.config.json',
    'ai.model': 'Model ID', 'ai.modelHint': 'Main model', 'ai.modelPh': 'e.g. gpt-4o-mini',
    'ai.backups': 'Backup models', 'ai.backupsHint': 'Used in order when the main model fails', 'ai.backupPh': 'Backup model ID',
    'ai.add': 'Add', 'ai.remove': 'Remove', 'ai.showKey': 'Show or hide key',
    'ai.connect': 'Connect', 'ai.discard': 'Discard changes',
    'ai.notTested': 'Not tested yet', 'ai.unsaved': 'Unsaved changes — press Connect to save', 'ai.testing': 'Connecting…',
    'ai.ok': 'Connected · {ms} ms · {host}',
    'ai.err.auth': 'API key rejected', 'ai.err.not_found': 'Model or endpoint not found', 'ai.err.rate': 'Rate limited — try again later',
    'ai.err.timeout': 'No response within 15 s', 'ai.err.network': 'Can’t reach the endpoint',
    'ai.err.bad_response': 'Endpoint replied, but not in OpenAI format', 'ai.err.http': 'Request failed',
    'ai.err.endpoint': 'Endpoint must start with http:// or https://', 'ai.err.model': 'Enter a model ID',
  },
  id: {
    'page.today': 'Hari ini', 'page.music': 'Musik', 'page.focus': 'Fokus', 'page.todo': 'Tugas',
    'page.water': 'Minum', 'page.shelf': 'File', 'page.chat': 'Obrolan', 'page.nav': 'Halaman Blip',
    'hdr.settings': 'Pengaturan', 'hdr.pin': 'Sematkan',

    'music.seek': 'Geser posisi', 'music.lyrics': 'Lirik', 'music.prev': 'Sebelumnya', 'music.toggle': 'Putar/Jeda',
    'music.next': 'Berikutnya', 'music.lyrSlower': 'Lirik lebih lambat', 'music.lyrFaster': 'Lirik lebih awal',
    'music.lyrReset': 'Klik dua kali untuk reset', 'music.untitled': 'musik',
    'lyr.loading': 'mencari lirik…', 'lyr.none': 'lirik tidak ditemukan', 'lyr.plain': 'lirik tanpa waktu (tidak tersinkron)',
    'lyr.error': 'lirik gagal dimuat · klik ikon untuk coba lagi',

    'focus.phase.focus': 'Fokus', 'focus.phase.short': 'Istirahat', 'focus.phase.long': 'Istirahat panjang',
    'focus.chip.focus': 'Fokus', 'focus.chip.short': 'Istirahat', 'focus.chip.long': 'Panjang',
    'focus.session': 'sesi {n}/4', 'focus.minus': 'Kurangi 5 menit', 'focus.plus': 'Tambah 5 menit',
    'focus.minutes': 'Menit', 'focus.min': 'mnt', 'focus.start': 'Mulai', 'focus.pause': 'Jeda',
    'focus.resume': 'Lanjut', 'focus.reset': 'Reset', 'focus.skip': 'Lewati',
    'focus.timeBreak': 'waktunya istirahat', 'focus.timeFocus': 'waktunya fokus',

    'todo.count': '{done} selesai hari ini · {left} tersisa', 'todo.clear': 'Hapus selesai', 'todo.placeholder': 'Tambah tugas…',
    'todo.add': 'Tambah', 'todo.empty': 'Belum ada tugas. Apa yang ingin kamu selesaikan?',
    'todo.remind': 'Ingatkan', 'todo.removeReminder': 'Hapus pengingat', 'todo.in15': '15 menit lagi', 'todo.in60': '1 jam lagi',
    'todo.chooseTime': 'Pilih tanggal & jam', 'todo.day': 'Tanggal', 'todo.month': 'Bulan', 'todo.year': 'Tahun', 'todo.hour': 'Jam', 'todo.minute': 'Menit',
    'todo.setReminder': 'Pasang', 'todo.invalidTime': 'Pilih tanggal yang valid dan waktu yang belum lewat.', 'todo.todayAt': 'Hari ini, {t}', 'todo.tomorrowAt': 'Besok, {t}',
    'todo.markOpen': 'Tandai belum', 'todo.markDone': 'Tandai selesai', 'todo.editHint': '{text} (klik dua kali untuk edit)',
    'todo.delete': 'Hapus', 'todo.tomorrow': 'besok {t}', 'todo.allDone': 'semua selesai',

    'water.undo': 'Batal', 'water.goalLbl': 'target', 'water.cupLbl': 'gelas', 'water.everyLbl': 'tiap',
    'water.goalReached': 'Target tercapai', 'water.pctToday': '{n}% hari ini', 'water.goalAlert': 'target minum tercapai',
    'water.remind': 'waktunya minum air',
    'today.focusSessions': '{n} sesi hari ini',

    'shelf.paste': 'Tempel', 'shelf.clear': 'Bersihkan', 'shelf.dropTitle': 'Tarik file ke sini',
    'shelf.dropHint': 'Tempel file dari clipboard, atau tarik ke sini', 'shelf.items': '{n} item',
    'shelf.pasteTo': 'Tempel ke app aktif', 'shelf.copied': 'disalin — Ctrl+V untuk tempel', 'shelf.open': 'Buka file',
    'shelf.more': 'Pilihan file', 'shelf.reveal': 'Buka lokasi', 'shelf.remove': 'Hapus dari Shelf',
    'shelf.notFound': 'file tidak ditemukan', 'shelf.noClip': 'clipboard tidak berisi file',

    'chat.placeholder': 'Tulis atau tekan Ctrl Kanan…', 'chat.speak': 'Bicara', 'chat.send': 'Kirim',
    'chat.qSum': 'ringkas clipboard', 'chat.qTr': 'terjemah clipboard', 'chat.qDay': 'hari ini', 'chat.clear': 'hapus',
    'chat.voiceOn': 'suara: on', 'chat.voiceOff': 'suara: off', 'chat.empty': '(kosong)', 'chat.thinking': 'berpikir...',
    'chat.speaking': 'bicara...', 'chat.failed': 'gagal', 'chat.failedMsg': 'gagal: {e}', 'chat.answerFail': 'gagal menjawab',
    'chat.lblSum': 'Ringkas clipboard', 'chat.lblTr': 'Terjemah clipboard', 'chat.lblDay': 'Ringkas hariku',
    'mem.ok': 'Oke, aku ingat.', 'mem.okLbl': 'diingat', 'mem.forgot': 'Sudah kulupakan semuanya.',
    'mem.forgotLbl': 'dilupakan', 'mem.none': 'Belum ada yang kuingat.', 'mem.recallLbl': 'ingatan',

    'voice.listenHint': 'ketuk jika selesai', 'voice.think': 'berpikir', 'voice.speakHint': 'ketuk untuk menyela',
    'voice.micFail': 'mic gagal', 'stt.empty': 'tidak terdengar', 'stt.fail': 'stt gagal', 'dictate.on': 'dikte...',

    'act.waiting': 'menunggu', 'act.working': 'bekerja...', 'act.timer': 'timer',
    'timer.done': 'timer selesai', 'timer.set': 'timer {t} dipasang', 'agent.done': '{a} selesai',

    'intent.next': 'lagu berikutnya', 'intent.prev': 'lagu sebelumnya', 'intent.pause': 'musik dijeda', 'intent.play': 'musik diputar',
    'intent.up': 'volume naik', 'intent.down': 'volume turun', 'intent.mute': 'volume mute', 'intent.water': '+{n} ml dicatat',
    'intent.focusStart': 'fokus dimulai', 'intent.focusStop': 'fokus dihentikan', 'intent.todo': 'tugas ditambah',
    'intent.set': 'volume diatur', 'intent.volume': 'volume {n}%', 'intent.opening': 'membuka {x}',

    'hud.mute': 'Bisu', 'hud.volume': 'Volume', 'hud.capsOn': 'Caps Lock nyala', 'hud.capsOff': 'Caps Lock mati',
    'hud.charging': 'Mengisi daya', 'hud.battLow': 'Baterai lemah', 'hud.unplugged': 'Charger dicabut',
    'hud.kbd': 'Bahasa keyboard', 'hud.netOn': 'Internet tersambung', 'hud.netOff': 'Internet terputus',
    'hud.audio': 'Perangkat audio', 'hud.bright': 'Kecerahan', 'hud.btOn': 'Terhubung', 'hud.btOff': 'Terputus',
    'batt.charging': 'mengisi daya', 'batt.battery': 'baterai',

    'greet.morning': 'Selamat pagi', 'greet.noon': 'Selamat siang', 'greet.afternoon': 'Selamat sore', 'greet.evening': 'Selamat malam',
    'nudge.battery': 'baterai {n}%, colok charger', 'nudge.night': 'sudah larut, jangan lupa istirahat',
    'nudge.evening': '{n} tugas belum selesai hari ini', 'nudge.greet.tasks': '{greet}. {n} tugas menunggu',
    'nudge.greet.none': '{greet}. belum ada tugas',

    'set.title': 'Pengaturan', 'set.nav.general': 'Umum', 'set.nav.ai': 'AI', 'set.nav.voice': 'Suara',
    'set.nav.notch': 'Notch', 'set.nav.pages': 'Halaman', 'set.nav.focus': 'Fokus & Air',
    'set.lang': 'Bahasa', 'set.langHint': 'Bahasa tampilan aplikasi',
    'set.autostart': 'Mulai saat login', 'set.autostartHint': 'Jalankan Blip otomatis',
    'set.hotkey': 'Hotkey buka/tutup', 'set.hotkeyHint': 'Ctrl+Alt+Q tetap untuk keluar',
    'set.dictate': 'Tombol dikte (tahan)', 'set.dictateHint': 'Tahan untuk bicara',
    'set.key.rctrl': 'Ctrl Kanan', 'set.key.ralt': 'Alt Kanan', 'set.off': 'Mati',
    'set.cfg': 'File config', 'set.cfgHint': 'Pengaturan lanjutan (prompt, header)', 'set.cfgOpen': 'Buka file config',
    'set.stt': 'Pengenal suara', 'set.sttHint': 'Groq butuh API key, Lokal pakai whisper', 'set.local': 'Lokal',
    'set.groqKey': 'Groq API key', 'set.groqKeyHint': 'Untuk pengenal suara Groq',
    'set.sttLang': 'Bahasa ucapan', 'set.sttLangHint': 'Kode bahasa, mis. id atau en',
    'set.tts': 'Bacakan jawaban', 'set.ttsHint': 'Bacakan jawaban AI',
    'set.engine': 'Mesin suara', 'set.engineHint': 'ElevenLabs jatuh ke Edge bila gagal',
    'set.edgeVoice': 'Suara Edge', 'set.edgeVoiceHint': 'mis. id-ID-ArdiNeural', 'set.optional': 'Opsional',
    'set.hover': 'Buka saat hover', 'set.hoverHint': 'Mati: klik notch untuk membuka',
    'set.closeDelay': 'Jeda sebelum tutup', 'set.closeDelayHint': 'Setelah kursor keluar',
    'set.fast': 'Cepat', 'set.normal': 'Normal', 'set.relaxed': 'Santai', 'set.slow': 'Lambat',
    'set.autoHide': 'Tutup jawaban otomatis', 'set.autoHideHint': 'Lama jawaban tampil', 'set.s3': '3 dtk', 'set.s5': '5 dtk', 'set.s8': '8 dtk',
    'set.fullscreen': 'Sembunyi saat app layar penuh', 'set.fullscreenHint': 'Game, video, presentasi',
    'set.compact': 'Ringkas saat jendela bertumpuk dengan Blip', 'set.compactHint': 'Mengecil jadi garis tipis; arahkan kursor ke tepi atas untuk membuka',
    'set.nudge': 'Pengingat (nudge)', 'set.nudgeHint': 'Baterai, minum, fokus',
    'set.motion': 'Gerakan', 'set.motionHint': 'Animasi buka/tutup', 'set.motionSystem': 'Ikuti Windows',
    'set.motionReduce': 'Kurangi', 'set.motionFull': 'Penuh', 'set.speed': 'Kecepatan animasi', 'set.speedHint': 'Durasi semua transisi',
    'set.pg.todayHint': 'Jam, tugas, minum, fokus', 'set.pg.musicHint': 'Hanya tampil saat media diputar',
    'set.pg.focusHint': 'Pomodoro', 'set.pg.todoHint': 'Daftar tugas', 'set.pg.waterHint': 'Pencatat air',
    'set.pg.shelfHint': 'Mati: file yang ditarik ke notch diabaikan', 'set.pg.chatHint': 'selalu tampil',
    'set.minutes': 'Menit', 'set.waterGoal': 'Target minum (ml)', 'set.cup': 'Ukuran gelas (ml)', 'set.every': 'Ingatkan tiap (menit)',
    'set.hours': 'Jam mulai / selesai', 'set.hoursHint': 'Pengingat hanya di rentang ini',
    'set.saved': 'Tersimpan', 'set.badNumber': 'Angka tidak valid', 'set.badHours': 'Jam selesai harus setelah jam mulai',

    'ai.endpoint': 'Endpoint', 'ai.endpointHint': 'API kompatibel OpenAI', 'ai.key': 'API key', 'ai.keyHint': 'Disimpan di blip.config.json',
    'ai.model': 'ID model', 'ai.modelHint': 'Model utama', 'ai.modelPh': 'mis. gpt-4o-mini',
    'ai.backups': 'Model cadangan', 'ai.backupsHint': 'Dipakai berurutan bila model utama gagal', 'ai.backupPh': 'ID model cadangan',
    'ai.add': 'Tambah', 'ai.remove': 'Hapus', 'ai.showKey': 'Tampilkan atau sembunyikan key',
    'ai.connect': 'Hubungkan', 'ai.discard': 'Buang perubahan',
    'ai.notTested': 'Belum diuji', 'ai.unsaved': 'Ada perubahan — tekan Hubungkan untuk menyimpan', 'ai.testing': 'Menghubungkan…',
    'ai.ok': 'Terhubung · {ms} ms · {host}',
    'ai.err.auth': 'API key ditolak', 'ai.err.not_found': 'Model atau endpoint tidak ditemukan', 'ai.err.rate': 'Kena batas — coba lagi nanti',
    'ai.err.timeout': 'Tidak ada respons dalam 15 dtk', 'ai.err.network': 'Endpoint tidak bisa dihubungi',
    'ai.err.bad_response': 'Endpoint membalas, tapi bukan format OpenAI', 'ai.err.http': 'Permintaan gagal',
    'ai.err.endpoint': 'Endpoint harus diawali http:// atau https://', 'ai.err.model': 'Isi ID model',
  },
};

let LANG = globalThis.__LANG === 'id' ? 'id' : 'en';

function t(key, vars) {
  const s = STR[LANG][key] ?? STR.en[key] ?? key;
  return vars ? s.replace(/\{(\w+)\}/g, (m, k) => (vars[k] == null ? m : String(vars[k]))) : s;
}
function setLang(l) { LANG = l === 'id' ? 'id' : 'en'; }
function lang() { return LANG; }

function applyI18n(root) {
  root = root || document;
  for (const el of root.querySelectorAll('[data-i18n]')) el.textContent = t(el.getAttribute('data-i18n'));
  for (const el of root.querySelectorAll('[data-i18n-title]')) el.setAttribute('title', t(el.getAttribute('data-i18n-title')));
  for (const el of root.querySelectorAll('[data-i18n-aria]')) el.setAttribute('aria-label', t(el.getAttribute('data-i18n-aria')));
  for (const el of root.querySelectorAll('[data-i18n-ph]')) el.setAttribute('placeholder', t(el.getAttribute('data-i18n-ph')));
  document.documentElement.setAttribute('lang', LANG);
}

const DATE_NAMES = {
  en: [['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'], ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']],
  id: [['Min', 'Sen', 'Sel', 'Rab', 'Kam', 'Jum', 'Sab'], ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des']],
};
function fmtDate(d) {
  const [wd, mo] = DATE_NAMES[LANG];
  return wd[d.getDay()] + ', ' + d.getDate() + ' ' + mo[d.getMonth()];
}

if (typeof module !== 'undefined' && module.exports) {
  module.exports = { STR, t, setLang, lang, fmtDate };
}
