__seedStorage({ 'blip.page': 'todo', 'blip.pageAt': Date.now(),
  'blip.todos': [
    { id: 't1', text: 'Kirim laporan', done: false, dueAt: new Date(2026, 9, 24, 14, 30).getTime(), notified: false },
    { id: 't2', text: 'Beli kopi', done: true, dueAt: null, notified: false, doneAt: Date.now() - 600000 },
    { id: 't3', text: 'Baca buku', done: false, dueAt: null, notified: false }
  ] });
setTimeout(() => __emit('hover', true), 100);
