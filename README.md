# Blip

Asisten AI berbentuk *dynamic island* untuk Windows. Arahkan kursor ke notch: panel membuka halaman Musik, Fokus, Tugas, Minum, Shelf, dan Obrolan (dot + panah, scroll wheel). Tekan tombol Copilot: island mendengar, menjawab dengan teks + suara, lalu mengecil. Saat mouse keluar, panel menutup kecuali sedang mengetik atau AI menjawab.

Dibangun dengan Tauri 2 (Rust + WebView2) — bukan Electron.

## Prasyarat

- Windows 10/11 dengan WebView2 (sudah ada di Windows 11)
- Rust: `winget install Rustlang.Rustup`, lalu `rustup default stable-x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools + C++ workload:
  `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`
- Node.js 18+ (hanya untuk Tauri CLI)
- `ffmpeg` dan `uv` (fallback STT lokal + edge-tts), `edge-tts` (`uv tool install edge-tts`)

## Jalankan

```
npm install
npm run dev
```

Build rilis:

```
npm run build
```

Hasil: `src-tauri\target\release\blip.exe`.

## Config

`%APPDATA%\Blip\blip.config.json` (atau path di env `BLIP_CONFIG`). Bentuk:

```json
{
  "router": { "baseUrl": "http://127.0.0.1:20128/v1", "apiKey": "", "models": ["oc/deepseek-v4.1-flash-free", "..."] },
  "stt":    { "provider": "groq", "apiKey": "", "model": "whisper-large-v3-turbo", "language": "id" },
  "tts":    { "enabled": true, "voice": "id-ID-ArdiNeural" },
  "ui":     { "port": 51321, "holdMs": 300, "autoCollapseMs": 5000 },
  "systemPrompt": "..."
}
```

- `router.apiKey` kosong -> dibaca dari `~/.hermes/config.yaml`.
- `stt.provider != "groq"` atau `apiKey` kosong -> whisper lokal via `uv run --with faster-whisper`.
- Perubahan config butuh restart app.

## Perilaku

| Aksi | Hasil |
|---|---|
| Tap tombol Copilot (Right Ctrl) | island mendengar, diam 1.3 dtk -> jawab + suara |
| Tahan >= `ui.holdMs` | dikte: teks ditempel ke app aktif, clipboard dipulihkan |
| Tap saat mendengar | kirim lebih awal |
| Tap saat menjawab | hentikan suara, mendengar lagi |
| Hover notch | buka panel halaman relevan (musik berjalan -> Musik, timer fokus -> Fokus, selain itu halaman terakhir); histeresis 18px/6px |
| Drag file dekat notch | zona magnet 520px bangunkan notch ke Shelf |
| Keluar hover | mengecil setelah 300 ms; tetap terbuka bila disematkan (pin), input berisi teks, sedang drag, atau AI menjawab |
| Tombol Pin | sematkan notch agar tetap terbuka |
| `‹` `›`, dot, wheel | pindah halaman dengan transisi geser halus |
| Musik | cover, judul, artis, badge player (Spotify/Brave/dll), seek bar, lirik otomatis split view (LRCLIB), klik baris = seek |
| Fokus | Pomodoro 25/5/15 dengan chips cepat, ring progres, sesi 1-4 |
| Tugas | to-do lokal + jam pengingat (`@14:30`), klik dua kali teks untuk edit, centang/hapus |
| Minum | target harian, `+250 ml`, grafik per 2 jam, pengingat saat jam aktif |
| Shelf | tarik file masuk/keluar, tombol Salin (`FileDropList`), Buka Lokasi di Explorer, Tempel (Ctrl+V) |
| HUD | volume, Caps Lock, charger/baterai, bahasa keyboard, status internet, perangkat audio |
| Full-screen | notch otomatis bersembunyi saat aplikasi atau game layar penuh berjalan |
| `Ctrl+Alt+B` | sama dengan tap tombol Copilot |
| `Ctrl+Alt+Q` | keluar |

Tray: Buka/Tutup, Mulai saat login, Keluar.

Perintah lokal (suara atau teks, tanpa LLM, maks 7 kata): `lagu berikutnya`/`skip`, `lagu sebelumnya`, `pause`, `lanjutkan musik`, `volume naik|turun|mute` (`banget` = 20%), `volume 50` (0-100), `buka spotify|youtube|youtube music|whatsapp|gmail|github|chatgpt`, `putar <judul> di youtube|spotify`, `timer 5 menit`, `minum`, `mulai fokus`/`stop fokus`, `tambah tugas <teks> [@14:30]`.

Catatan: tombol Copilot di laptop ini terpetakan ke Right Ctrl (VK 0xA3). Semua Right Ctrl diperlakukan sama.

## Server kontrol

`127.0.0.1:51321` (hanya loopback):

| Rute | Efek |
|---|---|
| `GET /health` | `{"ok":true}` |
| `POST /toggle` | tap hotkey |
| `POST /show` | buka + mendengar |
| `POST /hide` | tutup |
| `POST /ask` `{"text":"..."}` | tanya tanpa mic |
| `POST /agent` `{"agent":"claude","state":"working|waiting|done","text":"..."}` | notifikasi island |

Contoh hook Claude Code (`~/.claude/settings.json`):

```json
{
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "curl -s -X POST http://127.0.0.1:51321/agent -d \"{\\\"agent\\\":\\\"claude\\\",\\\"state\\\":\\\"working\\\"}\"" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "curl -s -X POST http://127.0.0.1:51321/agent -d \"{\\\"agent\\\":\\\"claude\\\",\\\"state\\\":\\\"waiting\\\"}\"" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "curl -s -X POST http://127.0.0.1:51321/agent -d \"{\\\"agent\\\":\\\"claude\\\",\\\"state\\\":\\\"done\\\"}\"" }] }]
  }
}
```

## Struktur

```
ui/                index.html, style.css, app.js (state machine island), lib.js (fungsi pure), app.test.js (`node ui/app.test.js`)
src-tauri/src/     main.rs, conf.rs, island.rs, keys.rs, rec.rs, ai.rs, media.rs (GSMTC + lirik), sys.rs (HUD audio/baterai, shelf), server.rs
scripts/stt_local.py
```
