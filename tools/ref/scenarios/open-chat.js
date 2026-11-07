__seedStorage({ 'blip.page': 'chat', 'blip.pageAt': Date.now(),
  'blip.chat': [
    { role: 'user', content: 'halo blip' },
    { role: 'assistant', content: 'Halo! Ada yang bisa dibantu?' },
    { role: 'user', content: 'jam berapa sekarang?' },
    { role: 'assistant', content: 'Sekarang jam 09.41.' }
  ] });
setTimeout(() => __emit('hover', true), 100);
