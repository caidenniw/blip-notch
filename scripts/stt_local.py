"""STT lokal (fallback kalau Groq tidak dipakai).
Dipakai main.js: uv run --with faster-whisper python scripts/stt_local.py <file> <lang>
"""
import sys
from faster_whisper import WhisperModel

path = sys.argv[1]
lang = sys.argv[2] if len(sys.argv) > 2 else "id"

model = WhisperModel("small", device="cpu", compute_type="int8")
segments, _ = model.transcribe(path, language=lang, beam_size=1, vad_filter=True)
print("".join(s.text for s in segments).strip())
