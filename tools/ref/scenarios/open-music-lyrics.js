__shotAt = 3500;
__seedStorage({ 'blip.page': 'music', 'blip.pageAt': Date.now() });
setTimeout(() => __emit('media', { title: 'Mirrors', artist: 'Justin Timberlake', playing: true, position: 30, duration: 480, app: 'Spotify.exe', ts: Date.now(), art: __ART }), 50);
setTimeout(() => __emit('hover', true), 600);
setTimeout(() => { if (!document.body.classList.contains('lyr')) __click('#lyrBtn'); }, 1500);
