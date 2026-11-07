// Browser surface for the original ui/app.js, on top of the native `__dom` bindings.
// Node ids are numbers; wrappers are cached per id so identity comparisons (===) hold.
'use strict';
(function (g) {
  const D = __dom;
  const wrappers = new Map();
  const listeners = new Map(); // id -> { type: [{fn, capture, once}] }
  const DOC = 0;              // pseudo id for `document`

  const camelToKebab = s => s.replace(/[A-Z]/g, c => '-' + c.toLowerCase());
  const kebabToCamel = s => s.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
  const wrap = id => {
    if (id == null || id < 0) return null;
    let w = wrappers.get(id);
    if (!w) {
      w = D.isText(id) ? new Text(id) : new Element(id);
      wrappers.set(id, w);
    }
    return w;
  };
  const ids = list => list.map(wrap);

  class EventTarget {
    addEventListener(type, fn, opts) {
      if (typeof fn !== 'function') return;
      const capture = opts === true || !!(opts && opts.capture);
      const once = !!(opts && opts.once);
      let m = listeners.get(this.__id);
      if (!m) listeners.set(this.__id, (m = {}));
      (m[type] || (m[type] = [])).push({ fn, capture, once });
    }
    removeEventListener(type, fn, opts) {
      const capture = opts === true || !!(opts && opts.capture);
      const m = listeners.get(this.__id);
      if (!m || !m[type]) return;
      m[type] = m[type].filter(l => !(l.fn === fn && l.capture === capture));
    }
    dispatchEvent(ev) { dispatch(this, ev); return !ev.defaultPrevented; }
  }

  class Node extends EventTarget {
    constructor(id) { super(); this.__id = id; }
    get parentNode() { return wrap(D.parent(this.__id)); }
    get parentElement() { return this.parentNode; }
    get textContent() { return D.text(this.__id); }
    set textContent(v) { D.setText(this.__id, v == null ? '' : String(v)); }
    get isConnected() { return D.connected(this.__id); }
    contains(o) { return !!o && D.contains(this.__id, o.__id); }
    remove() { D.remove(this.__id); }
  }
  class Text extends Node {
    get nodeType() { return 3; }
    get data() { return D.text(this.__id); }
    set data(v) { D.setText(this.__id, String(v)); }
  }

  class ClassList {
    constructor(id) { this.__id = id; }
    add(...c) { for (const x of c) D.classAdd(this.__id, String(x)); }
    remove(...c) { for (const x of c) D.classRemove(this.__id, String(x)); }
    toggle(c, force) { return D.classToggle(this.__id, String(c), force === undefined ? -1 : force ? 1 : 0); }
    contains(c) { return D.classHas(this.__id, String(c)); }
    get length() { return D.attr(this.__id, 'class') ? D.attr(this.__id, 'class').trim().split(/\s+/).length : 0; }
  }

  const styleProxy = id => new Proxy({}, {
    get(_, k) {
      if (k === 'setProperty') return (p, v) => D.styleSet(id, String(p), v == null ? '' : String(v));
      if (k === 'removeProperty') return p => { const o = D.styleGet(id, String(p)); D.styleRm(id, String(p)); return o; };
      if (k === 'getPropertyValue') return p => D.styleGet(id, String(p));
      if (typeof k !== 'string') return undefined;
      return D.styleGet(id, camelToKebab(k));
    },
    set(_, k, v) { D.styleSet(id, camelToKebab(String(k)), v == null ? '' : String(v)); return true; },
  });

  const datasetProxy = id => new Proxy({}, {
    get(_, k) {
      if (typeof k !== 'string') return undefined;
      const v = D.attr(id, 'data-' + camelToKebab(k));
      return v == null ? undefined : v;
    },
    set(_, k, v) { D.setAttr(id, 'data-' + camelToKebab(String(k)), String(v)); return true; },
    deleteProperty(_, k) { D.rmAttr(id, 'data-' + camelToKebab(String(k))); return true; },
    has(_, k) { return D.attr(id, 'data-' + camelToKebab(String(k))) != null; },
  });

  const attrProp = (name, bool) => ({
    get() { const v = D.attr(this.__id, name); return bool ? v != null : (v == null ? '' : v); },
    set(v) {
      if (bool) { if (v) D.setAttr(this.__id, name, ''); else D.rmAttr(this.__id, name); }
      else D.setAttr(this.__id, name, String(v));
    },
  });

  class Element extends Node {
    get nodeType() { return 1; }
    get tagName() { return D.tag(this.__id).toUpperCase(); }
    get classList() { return this.__cl || (this.__cl = new ClassList(this.__id)); }
    get style() { return this.__st || (this.__st = styleProxy(this.__id)); }
    get dataset() { return this.__ds || (this.__ds = datasetProxy(this.__id)); }
    get className() { return D.attr(this.__id, 'class') || ''; }
    set className(v) { D.setAttr(this.__id, 'class', String(v)); }
    get children() { return ids(D.children(this.__id)); }
    get childNodes() { return ids(D.childNodes(this.__id)); }
    get firstElementChild() { return this.children[0] || null; }
    get lastElementChild() { const c = this.children; return c[c.length - 1] || null; }
    get innerHTML() { return ''; }
    set innerHTML(v) { D.html(this.__id, String(v)); }
    get value() { return D.value(this.__id); }
    set value(v) { D.setValue(this.__id, v == null ? '' : String(v)); }
    get src() { return D.attr(this.__id, 'src') || ''; }
    set src(v) { D.setAttr(this.__id, 'src', String(v)); }
    get draggable() { return D.attr(this.__id, 'draggable') === 'true'; }
    set draggable(v) { D.setAttr(this.__id, 'draggable', v ? 'true' : 'false'); }
    getAttribute(n) { return D.attr(this.__id, String(n)); }
    setAttribute(n, v) { D.setAttr(this.__id, String(n), String(v)); }
    removeAttribute(n) { D.rmAttr(this.__id, String(n)); }
    hasAttribute(n) { return D.attr(this.__id, String(n)) != null; }
    querySelector(s) { return wrap(D.query(this.__id, String(s), false)[0]); }
    querySelectorAll(s) { return ids(D.query(this.__id, String(s), true)); }
    matches(s) { return D.matches(this.__id, String(s)); }
    closest(s) { return wrap(D.closest(this.__id, String(s))); }
    appendChild(c) { D.append(this.__id, c.__id); return c; }
    append(...cs) {
      for (const c of cs) {
        if (typeof c === 'string') D.append(this.__id, D.createText(c));
        else D.append(this.__id, c.__id);
      }
    }
    insertBefore(c, ref) { D.insertBefore(this.__id, c.__id, ref ? ref.__id : -1); return c; }
    removeChild(c) { D.remove(c.__id); return c; }
    replaceWith(n) { D.replace(this.__id, n.__id); }
    get offsetTop() { return D.offset(this.__id)[0]; }
    get offsetLeft() { return D.offset(this.__id)[1]; }
    get offsetWidth() { return D.offset(this.__id)[2]; }
    get offsetHeight() { return D.offset(this.__id)[3]; }
    get clientHeight() { return D.scroll(this.__id)[2]; }
    get clientWidth() { return D.scroll(this.__id)[4]; }
    get scrollHeight() { return D.scroll(this.__id)[1]; }
    get scrollTop() { return D.scroll(this.__id)[0]; }
    set scrollTop(v) { D.setScrollTop(this.__id, Number(v) || 0, false); }
    scrollTo(o) {
      if (typeof o === 'object' && o) D.setScrollTop(this.__id, Number(o.top) || 0, o.behavior === 'smooth');
      else D.setScrollTop(this.__id, Number(arguments[1]) || 0, false);
    }
    getBoundingClientRect() {
      const [left, top, width, height] = D.rect(this.__id);
      return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top };
    }
    setPointerCapture() { D.capture(this.__id); }
    releasePointerCapture() { D.capture(-1); }
    hasPointerCapture() { return D.captured() === this.__id; }
    focus() { D.focus(this.__id); }
    blur() { D.blur(this.__id); }
    click() { dispatch(this, new MouseEvent('click', { bubbles: true, cancelable: true })); }
  }
  for (const [p, a, b] of [['id', 'id'], ['title', 'title'], ['type', 'type'], ['placeholder', 'placeholder'], ['hidden', 'hidden', true], ['alt', 'alt']]) {
    Object.defineProperty(Element.prototype, p, attrProp(a, b));
  }

  // ------------------------------------------------------------------ events
  class Event {
    constructor(type, init) {
      init = init || {};
      this.type = type;
      this.bubbles = !!init.bubbles;
      this.cancelable = !!init.cancelable;
      this.defaultPrevented = false;
      this.target = null;
      this.currentTarget = null;
      this.timeStamp = performance.now();
      this.__stop = false;
      this.__stopNow = false;
      for (const k of Object.keys(init)) if (!(k in this)) this[k] = init[k];
    }
    preventDefault() { if (this.cancelable) this.defaultPrevented = true; }
    stopPropagation() { this.__stop = true; }
    stopImmediatePropagation() { this.__stop = this.__stopNow = true; }
  }
  class MouseEvent extends Event {
    constructor(type, init) {
      super(type, init);
      init = init || {};
      for (const [k, d] of [['clientX', 0], ['clientY', 0], ['button', 0], ['buttons', 0], ['ctrlKey', false], ['shiftKey', false], ['altKey', false], ['metaKey', false], ['detail', 0]]) {
        this[k] = init[k] === undefined ? d : init[k];
      }
    }
  }
  class PointerEvent extends MouseEvent {
    constructor(type, init) {
      super(type, init);
      init = init || {};
      this.pointerId = init.pointerId === undefined ? 1 : init.pointerId;
      this.isPrimary = init.isPrimary === undefined ? true : init.isPrimary;
      this.pointerType = 'mouse';
    }
  }
  class KeyboardEvent extends Event {
    constructor(type, init) {
      super(type, init);
      init = init || {};
      for (const [k, d] of [['key', ''], ['code', ''], ['ctrlKey', false], ['shiftKey', false], ['altKey', false], ['metaKey', false], ['repeat', false]]) {
        this[k] = init[k] === undefined ? d : init[k];
      }
    }
  }
  class WheelEvent extends MouseEvent {
    constructor(type, init) {
      super(type, init);
      init = init || {};
      this.deltaX = init.deltaX || 0;
      this.deltaY = init.deltaY || 0;
      this.deltaMode = 0;
    }
  }

  function invokeList(node, ev, capturePhase) {
    const m = listeners.get(node.__id);
    const list = m && m[ev.type];
    if (!list || !list.length) return;
    ev.currentTarget = node;
    for (const l of list.slice()) {
      if (l.capture !== capturePhase && ev.eventPhase !== 2) continue;
      if (l.once) node.removeEventListener(ev.type, l.fn, l.capture);
      try { l.fn.call(node, ev); } catch (e) { __reportError(e); }
      if (ev.__stopNow) return;
    }
  }

  function dispatch(target, ev) {
    ev.target = target;
    const path = [];
    if (target !== document) {
      for (let n = target; n; n = n.parentNode) path.push(n);
      if (D.connected(target.__id)) path.push(document);
    } else path.push(document);
    path.push(g); // window
    for (let i = path.length - 1; i > 0 && !ev.__stop; i--) { ev.eventPhase = 1; invokeList(path[i], ev, true); }
    if (!ev.__stop) { ev.eventPhase = 2; invokeList(path[0], ev, false); }
    if (ev.bubbles) for (let i = 1; i < path.length && !ev.__stop; i++) { ev.eventPhase = 3; invokeList(path[i], ev, false); }
    ev.currentTarget = null;
    return !ev.defaultPrevented;
  }

  // Called by Rust: returns true when not prevented.
  g.__fire = function (id, type, init) {
    const t = id === DOC ? document : wrap(id);
    if (!t) return true;
    let ev;
    if (type.startsWith('pointer') || type === 'click' || type === 'dblclick' || type === 'dragstart') ev = new PointerEvent(type, init);
    else if (type === 'wheel') ev = new WheelEvent(type, init);
    else if (type === 'keydown' || type === 'keyup') ev = new KeyboardEvent(type, init);
    else ev = new Event(type, init);
    return dispatch(t, ev);
  };
  g.__forget = function (list) {
    for (const id of list) { wrappers.delete(id); listeners.delete(id); }
  };

  // ------------------------------------------------------------------ document / window
  class Document extends EventTarget {
    constructor() { super(); this.__id = DOC; }
    get body() { return wrap(D.body()); }
    get documentElement() { return wrap(D.root()); }
    get activeElement() { return wrap(D.active()) || this.body; }
    get parentNode() { return null; }
    querySelector(s) { return wrap(D.query(D.root(), String(s), false)[0]); }
    querySelectorAll(s) { return ids(D.query(D.root(), String(s), true)); }
    getElementById(i) { return wrap(D.query(D.root(), '#' + i, false)[0]); }
    createElement(tag) {
      tag = String(tag).toLowerCase();
      if (tag === 'canvas') return new Canvas();
      return wrap(D.create(tag));
    }
    createTextNode(s) { return wrap(D.createText(String(s))); }
    contains(o) { return !!o && D.connected(o.__id); }
  }
  const document = new Document();

  class Canvas {
    constructor() { this.width = 300; this.height = 150; }
    getContext() {
      return {
        drawImage(img) { this._src = img && img.src; },
        getImageData() { return { data: (this._src && D.imgPixels(this._src)) || new Array(256).fill(0) }; },
      };
    }
  }
  class Image {
    constructor() { this.onload = null; this.onerror = null; this._src = ''; }
    get src() { return this._src; }
    set src(v) {
      this._src = String(v);
      setTimeout(() => {
        const ok = !!D.imgPixels(this._src);
        const cb = ok ? this.onload : this.onerror;
        if (cb) cb.call(this, new Event(ok ? 'load' : 'error'));
      }, 0);
    }
  }

  // ------------------------------------------------------------------ timers
  const timers = new Map();
  let timerSeq = 0;
  g.setTimeout = (fn, ms, ...args) => { const id = ++timerSeq; timers.set(id, { fn, args, every: 0 }); D.timer(id, Math.max(0, Number(ms) || 0)); return id; };
  g.setInterval = (fn, ms, ...args) => { const id = ++timerSeq; const every = Math.max(1, Number(ms) || 0); timers.set(id, { fn, args, every }); D.timer(id, every); return id; };
  g.clearTimeout = g.clearInterval = id => { if (timers.delete(id)) D.clearTimer(id); };
  g.__timer = id => {
    const t = timers.get(id);
    if (!t) return;
    if (t.every) D.timer(id, t.every); else timers.delete(id);
    if (typeof t.fn === 'function') t.fn(...t.args);
  };
  let rafs = new Map(), rafSeq = 0;
  g.requestAnimationFrame = fn => { const id = ++rafSeq; rafs.set(id, fn); D.wantFrame(); return id; };
  g.cancelAnimationFrame = id => { rafs.delete(id); };
  g.__frame = ts => {
    const list = rafs; rafs = new Map();
    for (const fn of list.values()) { try { fn(ts); } catch (e) { __reportError(e); } }
  };
  g.performance = { now: () => D.now() };
  g.queueMicrotask = fn => Promise.resolve().then(fn);

  // ------------------------------------------------------------------ observers
  class ResizeObserver { constructor(cb) { this.cb = cb; } observe() {} unobserve() {} disconnect() {} }
  const mutObs = [];
  class MutationObserver {
    constructor(cb) { this.cb = cb; this.targets = []; }
    observe(t, o) { this.targets.push({ id: t.__id, o: o || {} }); if (!mutObs.includes(this)) mutObs.push(this); }
    disconnect() { this.targets = []; }
    takeRecords() { return []; }
  }
  // Rust passes [[targetId, kind], ...] with kind 'childList'|'characterData'|'attributes'.
  g.__mutations = function (recs) {
    for (const mo of mutObs) {
      const hit = recs.filter(([id, kind]) => mo.targets.some(({ id: t, o }) =>
        (o[kind] || (kind === 'characterData' && o.characterData)) && (id === t || (o.subtree && D.contains(t, id)))));
      if (hit.length) {
        try { mo.cb(hit.map(([id, kind]) => ({ type: kind, target: wrap(id) })), mo); } catch (e) { __reportError(e); }
      }
    }
  };
  g.__hasMutationObservers = () => mutObs.some(m => m.targets.length);

  // ------------------------------------------------------------------ misc
  g.getComputedStyle = (el, pseudo) => new Proxy({}, {
    get(_, k) {
      if (k === 'getPropertyValue') return p => D.computed(el.__id, pseudo || '', String(p));
      return typeof k === 'string' ? D.computed(el.__id, pseudo || '', camelToKebab(k)) : undefined;
    },
  });
  g.localStorage = {
    getItem: k => D.lsGet(String(k)),
    setItem: (k, v) => D.lsSet(String(k), String(v)),
    removeItem: k => D.lsRm(String(k)),
    clear: () => D.lsClear(),
    key: i => D.lsKeys()[i] ?? null,
    get length() { return D.lsKeys().length; },
  };
  const fmt = a => a.map(x => typeof x === 'string' ? x : (() => { try { return JSON.stringify(x); } catch (_) { return String(x); } })()).join(' ');
  g.console = { log: (...a) => D.log(fmt(a)), info: (...a) => D.log(fmt(a)), warn: (...a) => D.log('warn: ' + fmt(a)), error: (...a) => D.log('error: ' + fmt(a)), debug() {} };
  g.__reportError = e => D.log('js error: ' + (e && e.message ? e.message : String(e)) + '\n' + (e && e.stack ? e.stack : ''));

  const WD = ['Min', 'Sen', 'Sel', 'Rab', 'Kam', 'Jum', 'Sab'];
  const MO = ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des'];
  const origLDS = Date.prototype.toLocaleDateString;
  Date.prototype.toLocaleDateString = function (loc, o) {
    if (loc === 'id-ID' && o && o.weekday === 'short' && o.day === 'numeric' && o.month === 'short') {
      return WD[this.getDay()] + ', ' + this.getDate() + ' ' + MO[this.getMonth()];
    }
    return origLDS.call(this, loc, o);
  };

  Object.assign(g, {
    window: g, self: g, document, Node, Element, Text, HTMLElement: Element, EventTarget,
    Event, MouseEvent, PointerEvent, KeyboardEvent, WheelEvent, Image, ResizeObserver, MutationObserver,
    navigator: { userAgent: 'Blip', language: 'id-ID' },
    matchMedia: q => ({ matches: q.includes('reduce') ? D.reducedMotion() : false, addEventListener() {}, removeEventListener() {} }),
  });
  g.addEventListener = EventTarget.prototype.addEventListener;
  g.removeEventListener = EventTarget.prototype.removeEventListener;
  g.__id = -2;
})(globalThis);
