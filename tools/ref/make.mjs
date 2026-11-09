// Builds target/ref/<name>.html per scenario and screenshots it with headless Edge.
// CDP waits for page load, advances virtual time, and captures an explicitly committed frame.
// Usage: node tools/ref/make.mjs [scenario ...]
import { readFileSync, writeFileSync, readdirSync, mkdirSync, rmSync, existsSync } from 'node:fs';
import { capture } from './capture.mjs';
import { join, resolve, dirname } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const edge = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const out = join(root, 'target', 'ref');
const scenDir = join(root, 'tools', 'ref', 'scenarios');
mkdirSync(out, { recursive: true });

const url = p => pathToFileURL(join(root, p)).href;
const tag = p => `<script src="${url(p)}"></script>`;
const html = readFileSync(join(root, 'ui', 'index.html'), 'utf8');

const names = process.argv.slice(2).length
  ? process.argv.slice(2)
  : readdirSync(scenDir).filter(f => f.endsWith('.js')).map(f => f.slice(0, -3)).sort();

function pngSize(file) {
  const b = readFileSync(file);
  return [b.readUInt32BE(16), b.readUInt32BE(20)];
}

let failed = 0;
for (const name of names) {
  const src = readFileSync(join(scenDir, name + '.js'), 'utf8');
  const shotAt = Number((src.match(/__shotAt\s*=\s*(\d+)/) || [])[1] || 3000);
  const scripts = ['tools/ref/fixed.js', 'tools/ref/mock.js', `tools/ref/scenarios/${name}.js`, 'ui/i18n.js', 'ui/lib.js', 'ui/app.js']
    .map(tag).join('\n  ');
  const page = html
    .replace(/<link rel="stylesheet" href="style.css"\s*\/?>/, `<link rel="stylesheet" href="${url('ui/style.css')}" />`)
    .replace(/<script src="i18n.js"><\/script>\s*<script src="lib.js"><\/script>\s*<script src="app.js"><\/script>/, scripts);
  if (page === html || !page.includes('fixed.js') || !page.includes(url('ui/style.css'))) throw new Error('index.html shape changed');
  const htmlFile = join(out, name + '.html');
  const png = join(out, name + '.edge.png');
  writeFileSync(htmlFile, page);
  if (existsSync(png)) rmSync(png);
  const profile = join(out, `edge-profile-${name}`);
  rmSync(profile, { recursive: true, force: true });
  const result = await capture(edge, profile, pathToFileURL(htmlFile).href, shotAt);
  writeFileSync(png, result.bytes);
  console.log(`${name}: ${result.state}`);
  if (!existsSync(png)) { console.log(`${name}: FAILED (no screenshot)`); failed++; continue; }
  const [w, h] = pngSize(png);
  const ok = w === 640 && h === 460;
  if (!ok) failed++;
  console.log(`${name}: ${ok ? 'ok' : 'BAD SIZE'} ${w}x${h} @${shotAt}ms -> ${png}`);
}
process.exit(failed ? 1 : 0);
