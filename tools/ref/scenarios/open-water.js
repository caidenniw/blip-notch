__seedStorage({ 'blip.page': 'water', 'blip.pageAt': Date.now(),
  'blip.water': { goal: 2000, cup: 250, start: 8, end: 22, every: 60, lastNudge: 0, log: { '2026-10-24': [
    { t: new Date(2026, 9, 24, 7, 30).getTime(), ml: 250 },
    { t: new Date(2026, 9, 24, 8, 15).getTime(), ml: 250 },
    { t: new Date(2026, 9, 24, 9, 0).getTime(), ml: 250 }
  ] } } });
setTimeout(() => __emit('hover', true), 100);
