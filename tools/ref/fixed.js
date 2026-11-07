// Deterministic clock, RNG and storage for parity screenshots (Edge + native).
(function () {
  var RealDate = Date;
  var base = new RealDate(2026, 9, 24, 9, 41, 0, 0).getTime();
  var t0 = performance.now();
  function nowMs() { return base + (performance.now() - t0); }
  class FixedDate extends RealDate {
    constructor() {
      if (arguments.length === 0) super(nowMs());
      else super(...arguments);
    }
    static now() { return Math.floor(nowMs()); }
  }
  FixedDate.UTC = RealDate.UTC;
  FixedDate.parse = RealDate.parse;
  window.Date = FixedDate;

  var seed = 42;
  Math.random = function () {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 4294967296;
  };

  // CDP virtual time does not guarantee a compositor frame. Keep pending rAF callbacks so
  // capture can explicitly deliver one frame (native snapshot pumps already do this).
  var raf = window.requestAnimationFrame.bind(window);
  var cancelRaf = window.cancelAnimationFrame.bind(window);
  var frames = new Map();
  window.requestAnimationFrame = function (fn) {
    var id = raf(function (time) { frames.delete(id); fn(time); });
    frames.set(id, fn);
    return id;
  };
  window.cancelAnimationFrame = function (id) { frames.delete(id); cancelRaf(id); };
  window.__flushReferenceFrame = function () {
    var pending = Array.from(frames.entries());
    frames.clear();
    for (var entry of pending) { cancelRaf(entry[0]); entry[1](performance.now()); }
  };

  localStorage.clear();
  window.__seedStorage = function (obj) {
    localStorage.clear();
    for (var k in obj) localStorage.setItem(k, JSON.stringify(obj[k]));
  };
  window.__click = function (sel) {
    document.querySelector(sel).dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  };
  window.__shotAt = 3000;
})();
