// Builds every VELIORA logo variant from the geometry in symbol.mjs / wordmark.mjs.
import { VIEW, LEFT, RIGHT, FOLD, INK } from './symbol.mjs';
import { wordmark } from './wordmark.mjs';

// ── Brand palette ─────────────────────────────────────────────────────────
export const COLORS = {
  midnight: '#101521',
  deepNavy: '#151E30',
  glass: '#202B40',
  ice: '#8DBDFF',
  electric: '#65A9FF',
  lavender: '#A99BFF',
  softWhite: '#F4F7FC',
  secondary: '#A7B4C9',
  ink: '#121933', // wordmark on light backgrounds
};

// Gradient sets. "light" = for light backgrounds (deep navy base, as in the
// reference); "dark" = for dark/glass backgrounds (base lifted to indigo so the
// ribbon never sinks into Midnight).
const TONES = {
  light: {
    left: [['0', '#BCDDFF'], ['0.42', '#78ADF4'], ['0.78', '#3A55A8'], ['1', '#232E6B']],
    right: [['0', '#E3DCFF'], ['0.55', '#AB9BFF'], ['1', '#8573F0']],
    lens: [['0', '#F3F6FF', 0.97], ['0.6', '#DCE2FF', 0.95], ['1', '#B8BEFF', 0.95]],
    rim: 0.55,
    fold: 0.42,
  },
  dark: {
    left: [['0', '#D2EAFF'], ['0.42', '#86B9FF'], ['0.78', '#4F6BD6'], ['1', '#3946A6']],
    right: [['0', '#E6E0FF'], ['0.55', '#B3A5FF'], ['1', '#8E7CFA']],
    lens: [['0', '#F4F7FF', 0.97], ['0.6', '#DEE3FF', 0.95], ['1', '#BDC2FF', 0.95]],
    rim: 0.4,
    fold: 0.58,
  },
};

const stops = (list) =>
  list
    .map(([o, c, a]) => `<stop offset="${o}" stop-color="${c}"${a != null ? ` stop-opacity="${a}"` : ''}/>`)
    .join('');

/** Full-colour symbol body (no <svg> wrapper). idp = id prefix (keeps ids unique when inlined). */
export function symbolBody(tone = 'light', idp = 'v') {
  const t = TONES[tone];
  return `<defs>
<linearGradient id="${idp}L" x1="120" y1="104" x2="292" y2="432" gradientUnits="userSpaceOnUse">${stops(t.left)}</linearGradient>
<linearGradient id="${idp}R" x1="420" y1="104" x2="300" y2="366" gradientUnits="userSpaceOnUse">${stops(t.right)}</linearGradient>
<linearGradient id="${idp}N" x1="262" y1="250" x2="320" y2="396" gradientUnits="userSpaceOnUse">${stops(t.lens)}</linearGradient>
<linearGradient id="${idp}F" x1="120" y1="104" x2="210" y2="200" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#FFFFFF" stop-opacity="${t.fold}"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0.05"/></linearGradient>
<linearGradient id="${idp}S" x1="0" y1="104" x2="0" y2="200" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#FFFFFF" stop-opacity="${t.rim}"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/></linearGradient>
<clipPath id="${idp}C"><path d="${LEFT}"/></clipPath>
</defs>
<path d="${RIGHT}" fill="url(#${idp}R)"/>
<path d="${LEFT}" fill="url(#${idp}L)"/>
<path d="${FOLD}" fill="url(#${idp}F)"/>
<path d="${RIGHT}" fill="url(#${idp}N)" clip-path="url(#${idp}C)"/>
<path d="${RIGHT}" fill="none" stroke="url(#${idp}S)" stroke-width="2"/>
<path d="${LEFT}" fill="none" stroke="url(#${idp}S)" stroke-width="2"/>`;
}

/** One-colour symbol: the left ribbon passes in front; a hairline gap separates it from the right one. */
export function symbolMonoBody(color, idp = 'm') {
  return `<defs>
<mask id="${idp}K" maskUnits="userSpaceOnUse" x="0" y="0" width="${VIEW}" height="${VIEW}">
<rect width="${VIEW}" height="${VIEW}" fill="#fff"/>
<path d="${LEFT}" fill="#000" stroke="#000" stroke-width="22" stroke-linejoin="round"/>
</mask>
</defs>
<path d="${RIGHT}" fill="${color}" mask="url(#${idp}K)"/>
<path d="${LEFT}" fill="${color}"/>`;
}

// ── wrappers ──────────────────────────────────────────────────────────────
const head = (w, h, title) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" role="img" aria-label="${title}"><title>${title}</title>`;

// Symbol is cropped to its optical box (with clear space) for standalone use.
const PADX = 28, PADY = 26;
const SYM_BOX = { x: INK.x0 - PADX, y: INK.y0 - PADY, w: INK.x1 - INK.x0 + PADX * 2, h: INK.y1 - INK.y0 + PADY * 2 };

export function symbolSvg(tone = 'light') {
  const body = tone.startsWith('mono-')
    ? symbolMonoBody(tone === 'mono-white' ? '#FFFFFF' : '#0B0F19')
    : symbolBody(tone);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${SYM_BOX.x} ${SYM_BOX.y} ${SYM_BOX.w} ${SYM_BOX.h}" width="${SYM_BOX.w}" height="${SYM_BOX.h}" role="img" aria-label="VELIORA"><title>VELIORA</title>${body}</svg>`;
}

/** Square app-icon tile: Midnight glass squircle with the symbol. */
export function appIconSvg({ small = false } = {}) {
  const S = 512;
  // symbol scaled into the tile with generous optical padding
  const scale = small ? 0.98 : 0.8;
  const cx = (INK.x0 + INK.x1) / 2, cy = (INK.y0 + INK.y1) / 2 + 4; // optical centre (V is top-heavy)
  const tx = S / 2 - cx * scale;
  const ty = S / 2 - cy * scale;
  const r = small ? 104 : 116;
  return `${head(S, S, 'VELIORA App')}
<defs>
<linearGradient id="tb" x1="0" y1="0" x2="0" y2="${S}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#202B40"/><stop offset="1" stop-color="#101521"/></linearGradient>
<radialGradient id="tg" cx="0.5" cy="0.62" r="0.62"><stop offset="0" stop-color="#65A9FF" stop-opacity="0.22"/><stop offset="1" stop-color="#65A9FF" stop-opacity="0"/></radialGradient>
<linearGradient id="te" x1="0" y1="0" x2="0" y2="${S}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#FFFFFF" stop-opacity="0.28"/><stop offset="0.5" stop-color="#FFFFFF" stop-opacity="0.04"/><stop offset="1" stop-color="#FFFFFF" stop-opacity="0.10"/></linearGradient>
</defs>
<rect x="8" y="8" width="${S - 16}" height="${S - 16}" rx="${r}" fill="url(#tb)"/>
<rect x="8" y="8" width="${S - 16}" height="${S - 16}" rx="${r}" fill="url(#tg)"/>
<rect x="9.5" y="9.5" width="${S - 19}" height="${S - 19}" rx="${r - 1.5}" fill="none" stroke="url(#te)" stroke-width="3"/>
<g transform="translate(${tx} ${ty}) scale(${scale})">${small ? symbolSmallBody() : symbolBody('dark', 'a')}</g>
</svg>`;
}

/** Simplified symbol for 16–48 px: no fold, no rims, flatter gradients, bigger lens contrast. */
const SMALL = {
  // in-tile (always on Midnight): light tops are fine
  tile: { l: ['#B9DCFF', '#4F6BD6'], r: ['#D9D0FF', '#9A88FF'], lens: '#EEF2FF' },
  // tray (unknown taskbar colour): deeper, so it holds on light AND dark taskbars
  tray: { l: ['#7DB4FF', '#3449B8'], r: ['#B6A8FF', '#7A64F2'], lens: '#E6EBFF' },
  // tray, nothing running: same mark in neutral slate — reads as "idle" on any taskbar
  trayOff: { l: ['#A3AEC2', '#5A6680'], r: ['#BCC3D1', '#727C93'], lens: '#E8EBF1' },
};
export function symbolSmallBody(idp = 's', kind = 'tile') {
  const c = SMALL[kind];
  return `<defs>
<linearGradient id="${idp}L" x1="120" y1="104" x2="292" y2="432" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="${c.l[0]}"/><stop offset="1" stop-color="${c.l[1]}"/></linearGradient>
<linearGradient id="${idp}R" x1="420" y1="104" x2="300" y2="366" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="${c.r[0]}"/><stop offset="1" stop-color="${c.r[1]}"/></linearGradient>
<clipPath id="${idp}C"><path d="${LEFT}"/></clipPath>
</defs>
<path d="${RIGHT}" fill="url(#${idp}R)"/>
<path d="${LEFT}" fill="url(#${idp}L)"/>
<path d="${RIGHT}" fill="${c.lens}" clip-path="url(#${idp}C)"/>`;
}

/** Tray icon: symbol only, transparent, simplified, tuned for 16/20/24/32 px on light & dark taskbars. */
export function traySvg(kind = 'tray') {
  const side = INK.x1 - INK.x0 + 12;
  const cy = (INK.y0 + INK.y1) / 2 + 4;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${INK.x0 - 6} ${cy - side / 2} ${side} ${side}" width="512" height="512">${symbolSmallBody('t', kind)}</svg>`;
}

/** Wordmark alone. */
export function wordmarkSvg(color) {
  const wm = wordmark();
  const pad = 8;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${-pad} ${-pad} ${wm.width + pad * 2} ${wm.height + pad * 2}" width="${wm.width + pad * 2}" height="${wm.height + pad * 2}" role="img" aria-label="VELIORA"><title>VELIORA</title><path d="${wm.d}" fill="${color}"/></svg>`;
}

/**
 * Horizontal lockup: symbol + wordmark.
 * Rules: symbol height = 2.8 × cap height; gap = 0.9 × cap height; wordmark
 * optically centred on the symbol's visual mass (slightly above geometric centre).
 */
export function lockupSvg(tone = 'light') {
  const wm = wordmark();
  const cap = wm.height; // 100
  const symH = cap * 2.8; // symbol visual height (ribbon top → base)
  const s = symH / (INK.y1 - INK.y0);
  const symW = (INK.x1 - INK.x0) * s;
  const gap = cap * 0.95;
  const pad = cap * 0.4;
  const W = pad + symW + gap + wm.width + pad;
  const H = pad + symH + pad;
  const symX = pad - INK.x0 * s;
  const symY = pad - INK.y0 * s;
  const wmX = pad + symW + gap;
  const wmY = pad + (symH - cap) / 2 - cap * 0.06;

  let symbol, wordColor;
  if (tone === 'mono-white') { symbol = symbolMonoBody('#FFFFFF', 'lm'); wordColor = '#FFFFFF'; }
  else if (tone === 'mono-black') { symbol = symbolMonoBody('#0B0F19', 'lm'); wordColor = '#0B0F19'; }
  else if (tone === 'dark') { symbol = symbolBody('dark', 'ld'); wordColor = COLORS.softWhite; }
  else { symbol = symbolBody('light', 'll'); wordColor = COLORS.ink; }

  const w = Math.ceil(W), h = Math.ceil(H);
  return `${head(w, h, 'VELIORA')}
<g transform="translate(${symX.toFixed(2)} ${symY.toFixed(2)}) scale(${s.toFixed(5)})">${symbol}</g>
<path transform="translate(${wmX.toFixed(2)} ${wmY.toFixed(2)})" d="${wm.d}" fill="${wordColor}"/>
</svg>`;
}
