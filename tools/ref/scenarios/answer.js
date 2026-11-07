// Start a real pending request, move its display to the dictation layer, then deliver a chunk.
// This exercises the closed-notch answer branch without modifying the original app.js.
setTimeout(() => __emit('ui:ask', { text: 'Jelaskan singkat apa itu notch.' }), 100);
setTimeout(() => __emit('dictate', 'start'), 900);
setTimeout(() => __emit('chat:chunk', {
  id: __lastInvoke.chat_send.id,
  delta: 'Notch adalah panel kecil di tepi layar untuk aktivitas dan bantuan cepat.',
}), 1700);
__shotAt = 4000;
