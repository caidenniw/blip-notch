// window.__TAURI__ mock with canned invoke results and an event emitter.
(function () {
  var subs = {};
  window.__lastInvoke = {};
  window.__ART = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGOQ3PEfAAK+AdGnj6U5AAAAAElFTkSuQmCC';
  function basename(p) { return String(p).split(/[\\/]/).pop(); }
  function ext(n) { var i = n.lastIndexOf('.'); return i > 0 ? n.slice(i + 1).toLowerCase() : ''; }
  function invoke(cmd, args) {
    window.__lastInvoke[cmd] = args;
    var r = null;
    if (cmd === 'info') r = { model: 'gpt-4o-mini', stt: 'groq', tts: true, ttsVoice: 'id-ID-ArdiNeural', routerReady: true, autoCollapseMs: 5000 };
    else if (cmd === 'sys_status') r = { volume: 40, muted: false, caps: false, battery: 80, plugged: true, lang: 'en', net: true, audio: 'Speakers' };
    else if (cmd === 'lyrics') r = { synced: '[00:01.00]Satu\n[00:05.00]Dua\n[00:09.00]Tiga', plain: null };
    else if (cmd === 'config_get') r = {
      router: { baseUrl: 'https://api.openai.com/v1', apiKey: 'sk-test', models: ['gpt-4o-mini'], headers: {} },
      stt: { provider: 'groq', apiKey: '', model: 'whisper-large-v3-turbo', language: 'id' },
      tts: { enabled: true, voice: 'id-ID-ArdiNeural', provider: 'edge', elevenApiKey: '', elevenVoiceId: '', elevenModel: 'eleven_multilingual_v2' },
      ui: { port: 51321, holdMs: 300, autoCollapseMs: 5000, toggleHotkey: 'ctrl+alt+b', dictateKey: 'rctrl', hideFullscreen: true, autoCompact: true, motion: 'system', animSpeed: 'normal' },
      systemPrompt: '',
    };
    else if (cmd === 'autostart_get') r = false;
    else if (cmd === 'router_test') r = ['gpt-4o', 'gpt-4o-mini'];
    else if (cmd === 'shelf_info') r = ((args && args.paths) || []).map(function (p) {
      var name = basename(p);
      return { path: p, name: name, ext: ext(name), size: 2516582, dir: false };
    });
    return Promise.resolve(r);
  }
  function listen(name, cb) {
    (subs[name] = subs[name] || []).push(cb);
    return Promise.resolve(function () {});
  }
  window.__emit = function (name, payload) {
    (subs[name] || []).slice().forEach(function (cb) { cb({ event: name, payload: payload }); });
  };
  window.__TAURI__ = { core: { invoke: invoke }, event: { listen: listen } };
})();
