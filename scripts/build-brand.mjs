// Generates the complete VELIORA brand kit from scripts/brand/*.
//   npm run brand
// Outputs:
//   assets/brand/*            — master SVG/PNG/ICO files (single source used by the app)
//   src-tauri/icons/*         — Tauri bundle + tray icons
//   src-tauri/installer/*     — NSIS installer artwork (BMP)
import { Resvg } from '@resvg/resvg-js';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  symbolSvg, lockupSvg, wordmarkSvg, appIconSvg, traySvg, symbolBody, COLORS,
} from './brand/compose.mjs';
import { wordmark } from './brand/wordmark.mjs';
import { INK } from './brand/symbol.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const BRAND = join(ROOT, 'assets', 'brand');
const ICONS = join(ROOT, 'src-tauri', 'icons');
const INSTALLER = join(ROOT, 'src-tauri', 'installer');
for (const d of [BRAND, ICONS, INSTALLER]) mkdirSync(d, { recursive: true });

const written = [];
const out = (path, data) => { writeFileSync(path, data); written.push(path.replace(ROOT + '\\', '').replace(ROOT + '/', '')); };

const render = (svg, width, height) =>
  new Resvg(svg, {
    fitTo: height ? { mode: 'height', value: height } : { mode: 'width', value: width },
    font: { loadSystemFonts: false },
    shapeRendering: 2, imageRendering: 0,
  }).render();

// ── 1. SVG masters ────────────────────────────────────────────────────────
const svgs = {
  'logo.svg': lockupSvg('light'),
  'logo-light.svg': lockupSvg('light'),
  'logo-dark.svg': lockupSvg('dark'),
  'logo-monochrome-white.svg': lockupSvg('mono-white'),
  'logo-monochrome-black.svg': lockupSvg('mono-black'),
  'symbol.svg': symbolSvg('light'),
  'symbol-dark.svg': symbolSvg('dark'),
  'symbol-monochrome-white.svg': symbolSvg('mono-white'),
  'symbol-monochrome-black.svg': symbolSvg('mono-black'),
  'wordmark-dark.svg': wordmarkSvg(COLORS.softWhite),
  'wordmark-light.svg': wordmarkSvg(COLORS.ink),
  'app-icon.svg': appIconSvg(),
  'app-icon-small.svg': appIconSvg({ small: true }),
  'tray-icon.svg': traySvg(),
  'tray-icon-off.svg': traySvg('trayOff'),
};
for (const [name, svg] of Object.entries(svgs)) out(join(BRAND, name), svg);

// ── 2. PNG exports (transparent) ──────────────────────────────────────────
const pngs = [
  ['logo-light.png', svgs['logo-light.svg'], 2400],
  ['logo-dark.png', svgs['logo-dark.svg'], 2400],
  ['logo-monochrome-white.png', svgs['logo-monochrome-white.svg'], 2400],
  ['logo-monochrome-black.png', svgs['logo-monochrome-black.svg'], 2400],
  ['symbol.png', svgs['symbol.svg'], 1024],
  ['symbol-dark.png', svgs['symbol-dark.svg'], 1024],
  ['symbol-monochrome-white.png', svgs['symbol-monochrome-white.svg'], 1024],
  ['symbol-monochrome-black.png', svgs['symbol-monochrome-black.svg'], 1024],
  ['app-icon.png', svgs['app-icon.svg'], 1024],
];
for (const [name, svg, w] of pngs) out(join(BRAND, name), render(svg, w).asPng());

// ── 3. App icon raster set: small sizes use the simplified symbol ─────────
const iconPng = (size) => render(size <= 48 ? svgs['app-icon-small.svg'] : svgs['app-icon.svg'], size);
const ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

/** ICO writer: ≤48 px as 32-bit BMP/DIB (max compatibility), larger as PNG. */
function buildIco(sizes, rasterFor) {
  const images = sizes.map((size) => {
    const img = rasterFor(size);
    if (size > 48) return { size, data: Buffer.from(img.asPng()) };
    const rgba = img.pixels; // RGBA, top-down
    const w = size, h = size;
    const header = Buffer.alloc(40);
    header.writeUInt32LE(40, 0); header.writeInt32LE(w, 4); header.writeInt32LE(h * 2, 8);
    header.writeUInt16LE(1, 12); header.writeUInt16LE(32, 14); header.writeUInt32LE(0, 16);
    const xor = Buffer.alloc(w * h * 4);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const s = (y * w + x) * 4, d = ((h - 1 - y) * w + x) * 4;
      xor[d] = rgba[s + 2]; xor[d + 1] = rgba[s + 1]; xor[d + 2] = rgba[s]; xor[d + 3] = rgba[s + 3];
    }
    const maskRow = Math.ceil(w / 32) * 4;
    const and = Buffer.alloc(maskRow * h); // all 0 → alpha channel decides
    return { size, data: Buffer.concat([header, xor, and]) };
  });
  const dir = Buffer.alloc(6 + 16 * images.length);
  dir.writeUInt16LE(0, 0); dir.writeUInt16LE(1, 2); dir.writeUInt16LE(images.length, 4);
  let offset = dir.length;
  images.forEach((im, i) => {
    const o = 6 + i * 16;
    dir.writeUInt8(im.size >= 256 ? 0 : im.size, o);
    dir.writeUInt8(im.size >= 256 ? 0 : im.size, o + 1);
    dir.writeUInt8(0, o + 2); dir.writeUInt8(0, o + 3);
    dir.writeUInt16LE(1, o + 4); dir.writeUInt16LE(32, o + 6);
    dir.writeUInt32LE(im.data.length, o + 8); dir.writeUInt32LE(offset, o + 12);
    offset += im.data.length;
  });
  return Buffer.concat([dir, ...images.map((i) => i.data)]);
}

const ico = buildIco(ICO_SIZES, iconPng);
out(join(BRAND, 'app-icon.ico'), ico);
out(join(ICONS, 'icon.ico'), ico);
out(join(ICONS, 'icon.png'), iconPng(512).asPng());
out(join(ICONS, '32x32.png'), iconPng(32).asPng());
out(join(ICONS, '128x128.png'), iconPng(128).asPng());
out(join(ICONS, '128x128@2x.png'), iconPng(256).asPng());

// Tray: transparent symbol, simplified; 32 px master (+ 64 for high-DPI).
out(join(ICONS, 'tray-icon.png'), render(svgs['tray-icon.svg'], 32).asPng());
out(join(ICONS, 'tray-icon@2x.png'), render(svgs['tray-icon.svg'], 64).asPng());
out(join(BRAND, 'tray-icon.png'), render(svgs['tray-icon.svg'], 64).asPng());
out(join(ICONS, 'tray-icon-off.png'), render(svgs['tray-icon-off.svg'], 32).asPng());
out(join(ICONS, 'tray-icon-off@2x.png'), render(svgs['tray-icon-off.svg'], 64).asPng());
out(join(BRAND, 'tray-icon-off.png'), render(svgs['tray-icon-off.svg'], 64).asPng());

// ── 4. NSIS installer artwork (24-bit BMP, no alpha) ──────────────────────
function bmp24(img) {
  const { width: w, height: h } = img;
  const px = img.pixels;
  const row = Math.ceil((w * 3) / 4) * 4;
  const buf = Buffer.alloc(54 + row * h);
  buf.write('BM', 0); buf.writeUInt32LE(buf.length, 2); buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14); buf.writeInt32LE(w, 18); buf.writeInt32LE(h, 22);
  buf.writeUInt16LE(1, 26); buf.writeUInt16LE(24, 28); buf.writeUInt32LE(row * h, 34);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const s = (y * w + x) * 4, d = 54 + (h - 1 - y) * row + x * 3;
    buf[d] = px[s + 2]; buf[d + 1] = px[s + 1]; buf[d + 2] = px[s];
  }
  return buf;
}

const wm = wordmark();
// Sidebar 164×314 (welcome / finish pages)
const sidebar = `<svg xmlns="http://www.w3.org/2000/svg" width="164" height="314" viewBox="0 0 164 314">
<defs>
<linearGradient id="bg" x1="0" y1="0" x2="0" y2="314" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#202B40"/><stop offset="1" stop-color="#101521"/></linearGradient>
<radialGradient id="gl" cx="82" cy="120" r="120" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#65A9FF" stop-opacity="0.20"/><stop offset="1" stop-color="#65A9FF" stop-opacity="0"/></radialGradient>
</defs>
<rect width="164" height="314" fill="url(#bg)"/><rect width="164" height="314" fill="url(#gl)"/>
<g transform="translate(${82 - ((INK.x0 + INK.x1) / 2) * 0.2} ${118 - ((INK.y0 + INK.y1) / 2) * 0.2}) scale(0.2)">${symbolBody('dark', 'sb')}</g>
<path transform="translate(${82 - (wm.width * 0.13) / 2} 196) scale(0.13)" d="${wm.d}" fill="#F4F7FC"/>
<text x="82" y="232" text-anchor="middle" font-family="Segoe UI, Arial" font-size="9" letter-spacing="2.4" fill="#A7B4C9">APP</text>
</svg>`;
// Header 150×57 (inner pages, right side)
const header = `<svg xmlns="http://www.w3.org/2000/svg" width="150" height="57" viewBox="0 0 150 57">
<rect width="150" height="57" fill="#FFFFFF"/>
<g transform="translate(${118 - ((INK.x0 + INK.x1) / 2) * 0.082} ${28.5 - ((INK.y0 + INK.y1) / 2) * 0.082}) scale(0.082)">${symbolBody('light', 'hd')}</g>
</svg>`;
// resvg needs a font for the tiny "APP" label: allow system fonts here only.
const sidebarImg = new Resvg(sidebar, { fitTo: { mode: 'original' }, font: { loadSystemFonts: true } }).render();
out(join(INSTALLER, 'sidebar.bmp'), bmp24(sidebarImg));
out(join(INSTALLER, 'header.bmp'), bmp24(render(header, 150)));
out(join(BRAND, 'installer-sidebar.png'), sidebarImg.asPng());

// ── 5. Manifest: geometry the UI needs to size logos by their ink, not their box
const symBox = { x: INK.x0 - 28, y: INK.y0 - 26, w: INK.x1 - INK.x0 + 56, h: INK.y1 - INK.y0 + 52 };
const manifest = {
  name: 'VELIORA', product: 'VELIORA App',
  symbol: { box: symBox, ink: { x: INK.x0, y: INK.y0, w: INK.x1 - INK.x0, h: INK.y1 - INK.y0 } },
  wordmark: { cap: 100, pad: 8, width: wm.width },
  colors: COLORS,
};
out(join(BRAND, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
out(join(ROOT, 'public', 'favicon.svg'), svgs['symbol-dark.svg']);

console.log(`VELIORA brand kit: ${written.length} files`);
for (const f of written) console.log('  ' + f);
