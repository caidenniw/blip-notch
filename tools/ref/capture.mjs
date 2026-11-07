import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

// The screenshot CLI can finish before a loaded page's compositor commits. CDP lets us
// wait for navigation, advance virtual time, then explicitly capture the resulting frame.
export async function capture(edge, profile, url, milliseconds) {
  mkdirSync(profile, { recursive: true });
  const child = spawn(edge, ['--headless=new', '--disable-gpu', '--hide-scrollbars',
    '--no-first-run', '--no-default-browser-check', '--allow-file-access-from-files',
    '--force-device-scale-factor=1', '--remote-debugging-port=0',
    `--user-data-dir=${profile}`, 'about:blank'], { stdio: 'ignore' });
  let ws;
  try {
    const portFile = join(profile, 'DevToolsActivePort');
    for (let i = 0; i < 200 && !existsSync(portFile); i++) await delay(50);
    if (!existsSync(portFile)) throw new Error('Edge CDP startup timed out');
    const port = Number(readFileSync(portFile, 'utf8').split('\n')[0]);
    const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    ws = new WebSocket(tabs.find(t => t.type === 'page').webSocketDebuggerUrl);
    await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
    let seq = 0;
    const pending = new Map(), events = new Map();
    ws.onmessage = ev => {
      const m = JSON.parse(ev.data);
      if (m.id) {
        const p = pending.get(m.id); pending.delete(m.id);
        if (p) m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result);
      } else {
        const callbacks = events.get(m.method) || []; events.delete(m.method);
        callbacks.forEach(fn => fn(m.params));
      }
    };
    const send = (method, params = {}) => new Promise((resolve, reject) => {
      const id = ++seq; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params }));
    });
    const once = method => new Promise(resolve => {
      const list = events.get(method) || []; list.push(resolve); events.set(method, list);
    });
    await send('Page.enable');
    await send('Emulation.setDeviceMetricsOverride', { width: 640, height: 460, deviceScaleFactor: 1, mobile: false });
    await send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 0, g: 0, b: 0, a: 0 } });
    const loaded = once('Page.loadEventFired');
    await send('Page.navigate', { url });
    await loaded;
    let elapsed = (await send('Runtime.evaluate', { expression: 'performance.now()', returnByValue: true })).result.value;
    while (elapsed < milliseconds) {
      const expired = once('Emulation.virtualTimeBudgetExpired');
      const step = Math.min(16, milliseconds - elapsed);
      await send('Emulation.setVirtualTimePolicy', { policy: 'advance', budget: Math.max(0.1, step) });
      await expired;
      await send('Runtime.evaluate', { expression: 'window.__flushReferenceFrame(); void document.body.offsetWidth' });
      elapsed += step;
    }
    // These are settled-state scenarios: every finite CSS effect ends before shotAt. Edge can
    // defer creating transitions until forced reflow, so finish those explicitly after reflow.
    await send('Runtime.evaluate', { expression: `(() => {
      void document.body.offsetWidth;
      for (const a of document.getAnimations()) {
        if (a.playState !== 'paused') {
          if (a.effect.getComputedTiming().iterations !== Infinity) a.finish();
          else { a.currentTime = Math.max(0, performance.now() - (a.startTime || 0)); a.pause(); }
        }
      }
      void document.body.offsetWidth;
    })()` });
    const state = await send('Runtime.evaluate', { expression: 'JSON.stringify({state:document.body.dataset.state,page:document.body.dataset.page,time:performance.now(),width:getComputedStyle(document.querySelector("#island")).width})', returnByValue: true });
    const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    return { bytes: Buffer.from(shot.data, 'base64'), state: state.result.value };
  } finally {
    if (ws) ws.close();
    child.kill();
    await new Promise(resolve => child.once('exit', resolve));
  }
}
