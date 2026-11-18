// VELIORA theme library. Each theme is a mood, not a recolour: its own glass
// density, light sources behind the glass, specular strength, text contrast,
// accent ribbon, status colours and cursor-trail colour.

export type Mode = 'light' | 'dark';
type RGBA = [string, number]; // hex + alpha

export interface ThemeDef {
  id: string;
  name: string;
  description: string;
  mode: Mode;
  /** Scene: base colour, vertical wash, four light sources behind the glass */
  bg: string;
  wash: [string, string];
  blobs: [RGBA, RGBA, RGBA, RGBA];
  /** Glass: tint, how much white the glass adds, blur and specular strength */
  surface: RGBA;
  fillK: number;
  blur: number;
  specular: number;
  /** Lines and text */
  ink: string;
  text: [string, string, string];
  onAccent: string;
  code: string;
  /** The two ribbons of the logo: light, mid, deep */
  accent: [string, string, string];
  ok: string;
  warn: string;
  bad: string;
  /** Opaque surfaces for menus/tooltips and the power orbs */
  menu: string;
  orb: string;
  shade: string;
  well: string;
  wellK: number;
  trail: string;
  /** How lively the background light is by default (0–1) */
  motion: number;
}

const LIGHT_STATUS = { ok: '#0f9e78', warn: '#b4751c', bad: '#d4405c' };
const DARK_STATUS = { ok: '#7fdcc0', warn: '#f2c98b', bad: '#ff98a8' };

export const THEMES: ThemeDef[] = [
  {
    id: 'liquid-light', name: 'Liquid Glass Light', mode: 'light', motion: 1,
    description: 'Главная стеклянная тема: белое многослойное стекло, голубые блики и мягкое свечение.',
    bg: '#eaf0fa', wash: ['#f6f9ff', '#dfe8f7'],
    blobs: [['#7cb8ff', 0.55], ['#c3b2ff', 0.5], ['#8ff0ff', 0.4], ['#ffd2ee', 0.3]],
    surface: ['#ffffff', 0.5], fillK: 5, blur: 30, specular: 1,
    ink: '#13213f', text: ['#0d1730', '#43506d', '#7a87a2'], onAccent: '#ffffff', code: '#1d3a78',
    accent: ['#4f9bff', '#3d7bff', '#8a6cff'], ...LIGHT_STATUS,
    menu: '#f7f9fe', orb: '#eef3fb', shade: '#4a5b85', well: '#ffffff', wellK: 1.6, trail: '#5b9dff',
  },
  {
    id: 'pure-white', name: 'Pure White', mode: 'light', motion: 0.15,
    description: 'Чистый белый минимализм: почти непрозрачные панели, тонкие линии, никакого шума.',
    bg: '#f7f8fa', wash: ['#ffffff', '#eef0f4'],
    blobs: [['#d9e4f5', 0.35], ['#ece7f7', 0.3], ['#e5f1f7', 0.25], ['#ffffff', 0.5]],
    surface: ['#ffffff', 0.86], fillK: 3, blur: 14, specular: 0.5,
    ink: '#141922', text: ['#0f1217', '#535b68', '#8a919d'], onAccent: '#ffffff', code: '#1a4fb4',
    accent: ['#3b82f6', '#2563eb', '#4f46e5'], ...LIGHT_STATUS,
    menu: '#ffffff', orb: '#f4f6f9', shade: '#5a6372', well: '#f3f5f8', wellK: 2.4, trail: '#8ea7cf',
  },
  {
    id: 'arctic', name: 'Arctic Blue', mode: 'light', motion: 0.8,
    description: 'Холодные голубые оттенки, ледяные градиенты и прозрачные поверхности.',
    bg: '#e4f1f8', wash: ['#f1fbff', '#d4e8f3'],
    blobs: [['#7fdcff', 0.5], ['#a9d0ff', 0.45], ['#d8fbff', 0.6], ['#6fbff0', 0.35]],
    surface: ['#f6fcff', 0.48], fillK: 5, blur: 26, specular: 1.1,
    ink: '#0c2a3d', text: ['#08202f', '#3b5e72', '#7795a6'], onAccent: '#ffffff', code: '#0b5f86',
    accent: ['#2cc0ec', '#0a95d2', '#3d7bd9'], ok: '#0c9c80', warn: '#b0711a', bad: '#cf3f5f',
    menu: '#f3fbff', orb: '#e8f5fb', shade: '#3d6680', well: '#ffffff', wellK: 1.5, trail: '#2fbcea',
  },
  {
    id: 'aurora', name: 'Aurora', mode: 'light', motion: 1,
    description: 'Северное сияние: голубые, сиреневые и розовые переливы за стеклом.',
    bg: '#efeefb', wash: ['#f8f6ff', '#e6ebfa'],
    blobs: [['#6fcfff', 0.5], ['#a98bff', 0.5], ['#ff92dc', 0.34], ['#7cf0c8', 0.36]],
    surface: ['#ffffff', 0.44], fillK: 5, blur: 32, specular: 1,
    ink: '#1c1840', text: ['#16122f', '#544f78', '#8984a8'], onAccent: '#ffffff', code: '#4b2fb0',
    accent: ['#4f9dff', '#8a6cff', '#d06cf0'], ...LIGHT_STATUS,
    menu: '#faf8ff', orb: '#f1effc', shade: '#5a4f8a', well: '#ffffff', wellK: 1.6, trail: '#9a74ff',
  },
  {
    id: 'pearl', name: 'Pearl', mode: 'light', motion: 0.45,
    description: 'Жемчуг и молоко: тёплое серебро, шампань и мягкий перламутровый отлив.',
    bg: '#f3f0ec', wash: ['#fbf9f6', '#e9e4dd'],
    blobs: [['#fff3e2', 0.75], ['#e4def0', 0.55], ['#f2ddd6', 0.5], ['#dce6ef', 0.45]],
    surface: ['#fffdfa', 0.58], fillK: 4, blur: 24, specular: 1.2,
    ink: '#2a241e', text: ['#211c17', '#665d54', '#9a9188'], onAccent: '#ffffff', code: '#6b4f2f',
    accent: ['#c3a07c', '#a3815f', '#8c7bb8'], ok: '#3d9b74', warn: '#b07422', bad: '#c84a5c',
    menu: '#fdfbf8', orb: '#f5f1ec', shade: '#6b5c4c', well: '#ffffff', wellK: 1.5, trail: '#c2a283',
  },
  {
    id: 'lavender', name: 'Soft Lavender', mode: 'light', motion: 0.7,
    description: 'Нежные сиреневые и фиолетовые оттенки, мягкие и спокойные.',
    bg: '#f1ecfb', wash: ['#faf7ff', '#e7defa'],
    blobs: [['#bfa6ff', 0.55], ['#e6c0ff', 0.45], ['#b3bfff', 0.4], ['#ffd0ec', 0.32]],
    surface: ['#fdfbff', 0.5], fillK: 5, blur: 28, specular: 1,
    ink: '#24193f', text: ['#1e1636', '#5d5480', '#9690b2'], onAccent: '#ffffff', code: '#5a35c4',
    accent: ['#a98bff', '#7d5cff', '#c27dff'], ...LIGHT_STATUS,
    menu: '#fbf9ff', orb: '#f2edfc', shade: '#5b4c8c', well: '#ffffff', wellK: 1.6, trail: '#a985ff',
  },
  {
    id: 'sky', name: 'Sky Glass', mode: 'light', motion: 0.9,
    description: 'Воздушная тема: небесно-голубое стекло максимальной прозрачности.',
    bg: '#e6f2ff', wash: ['#f3f9ff', '#d3e7ff'],
    blobs: [['#93cbff', 0.6], ['#c4e4ff', 0.6], ['#ffffff', 0.7], ['#9fbcff', 0.42]],
    surface: ['#ffffff', 0.34], fillK: 6, blur: 36, specular: 1.15,
    ink: '#0d2445', text: ['#0a1f3d', '#405e89', '#7c95b6'], onAccent: '#ffffff', code: '#1556b0',
    accent: ['#45a8ff', '#2b84ff', '#6c8dff'], ...LIGHT_STATUS,
    menu: '#f5faff', orb: '#eaf4ff', shade: '#3e5f8f', well: '#ffffff', wellK: 1.4, trail: '#4aa6ff',
  },
  {
    id: 'liquid-dark', name: 'Liquid Glass Dark', mode: 'dark', motion: 1,
    description: 'Фирменная тёмная: дымчатое стекло, холодные блики, голубая и лавандовая ленты.',
    bg: '#0e1220', wash: ['#151b2b', '#0a0d16'],
    blobs: [['#5a8dff', 0.22], ['#9a86ff', 0.19], ['#3fd4ff', 0.1], ['#ffffff', 0.04]],
    surface: ['#1a2235', 0.46], fillK: 1, blur: 28, specular: 1,
    ink: '#ffffff', text: ['#f4f7fc', '#a7b4c9', '#7a879d'], onAccent: '#0c1222', code: '#dbe6ff',
    accent: ['#8dbdff', '#65a9ff', '#a99bff'], ...DARK_STATUS,
    menu: '#19213a', orb: '#151d30', shade: '#04070e', well: '#080c16', wellK: 1, trail: '#8db4ff',
  },
  {
    id: 'midnight', name: 'Midnight', mode: 'dark', motion: 0.2,
    description: 'Глубокий чёрный и графит. Минимализм: серебряный акцент, никакого цвета.',
    bg: '#07080b', wash: ['#0d0e12', '#050608'],
    blobs: [['#ffffff', 0.045], ['#5b6478', 0.1], ['#ffffff', 0.025], ['#3a4150', 0.1]],
    surface: ['#16171c', 0.7], fillK: 0.8, blur: 18, specular: 0.7,
    ink: '#ffffff', text: ['#ececef', '#9a9ca6', '#64666f'], onAccent: '#0b0c10', code: '#d7d9e2',
    accent: ['#eef0f6', '#c4c8d4', '#9ea4b8'], ok: '#8fd8b8', warn: '#e6c48c', bad: '#f29aa8',
    menu: '#15161b', orb: '#111216', shade: '#000000', well: '#000000', wellK: 1, trail: '#d9dce8',
  },
  {
    id: 'deep-ocean', name: 'Deep Ocean', mode: 'dark', motion: 0.85,
    description: 'Глубина океана: насыщенный тёмно-синий с бирюзовым свечением.',
    bg: '#031424', wash: ['#06223d', '#020b16'],
    blobs: [['#0a7cff', 0.3], ['#00c2ff', 0.2], ['#1c3fc4', 0.26], ['#00ffd0', 0.08]],
    surface: ['#07294a', 0.46], fillK: 1, blur: 30, specular: 1,
    ink: '#d9f1ff', text: ['#e6f4ff', '#8fb3d1', '#5f829f'], onAccent: '#021222', code: '#bde8ff',
    accent: ['#4fdcff', '#1f9dff', '#41e3cf'], ok: '#5fe3c2', warn: '#f5cd86', bad: '#ff8fa3',
    menu: '#0a2742', orb: '#082038', shade: '#00050c', well: '#010b16', wellK: 1, trail: '#3ad0ff',
  },
  {
    id: 'violet-night', name: 'Violet Night', mode: 'dark', motion: 0.9,
    description: 'Тёмно-фиолетовая ночь с деликатными лиловыми огнями.',
    bg: '#0f0a1c', wash: ['#1a1131', '#090512'],
    blobs: [['#8a4dff', 0.26], ['#d14dff', 0.14], ['#4d5cff', 0.18], ['#ff6ad5', 0.07]],
    surface: ['#231739', 0.46], fillK: 1, blur: 28, specular: 1,
    ink: '#f1e8ff', text: ['#f4eeff', '#b4a7d6', '#7c709e'], onAccent: '#140a24', code: '#e3d4ff',
    accent: ['#c6a4ff', '#9b6bff', '#e48cff'], ok: '#8ee0c4', warn: '#f3c98e', bad: '#ff95b4',
    menu: '#1f1533', orb: '#1a1130', shade: '#05010c', well: '#07030f', wellK: 1, trail: '#b497cf',
  },
  {
    id: 'obsidian', name: 'Obsidian', mode: 'dark', motion: 0.35,
    description: 'Почти чёрный вулканический камень с тонкими острыми отражениями.',
    bg: '#050506', wash: ['#0c0c0f', '#030303'],
    blobs: [['#ffffff', 0.045], ['#6b7cff', 0.07], ['#ffffff', 0.03], ['#a3abcc', 0.06]],
    surface: ['#0f0f12', 0.6], fillK: 0.9, blur: 22, specular: 1.5,
    ink: '#ffffff', text: ['#f5f5f7', '#a1a1aa', '#6a6a72'], onAccent: '#08080a', code: '#e4e6ef',
    accent: ['#f4f5fa', '#c9cfe0', '#aab4ff'], ok: '#8fdcc0', warn: '#ebc78f', bad: '#f59aa8',
    menu: '#121215', orb: '#0d0d10', shade: '#000000', well: '#000000', wellK: 1, trail: '#e6e9ff',
  },
];

export const THEME_BY_ID: Record<string, ThemeDef> = Object.fromEntries(THEMES.map((t) => [t.id, t]));

export interface CustomTheme {
  mode: Mode;
  accent: string;
  accent2: string;
  background: string;
}

export const DEFAULT_CUSTOM: CustomTheme = { mode: 'dark', accent: '#7fd0ff', accent2: '#b48cff', background: '#101828' };

/** Builds a full theme from the four choices of the Custom theme. */
export function customTheme(c: CustomTheme): ThemeDef {
  const base = c.mode === 'light' ? THEME_BY_ID['liquid-light'] : THEME_BY_ID['liquid-dark'];
  const mid = mixHex(c.accent, c.accent2, 0.5);
  return {
    ...base,
    id: 'custom',
    name: 'Своя тема',
    description: 'Ваши цвета на основе стеклянной темы.',
    bg: c.background,
    wash: [mixHex(c.background, c.mode === 'light' ? '#ffffff' : '#ffffff', c.mode === 'light' ? 0.5 : 0.04), mixHex(c.background, '#000000', c.mode === 'light' ? 0.04 : 0.3)],
    blobs: [[c.accent, c.mode === 'light' ? 0.5 : 0.22], [c.accent2, c.mode === 'light' ? 0.45 : 0.19], [mid, c.mode === 'light' ? 0.35 : 0.1], ['#ffffff', c.mode === 'light' ? 0.5 : 0.04]],
    surface: c.mode === 'light' ? ['#ffffff', 0.5] : [mixHex(c.background, '#ffffff', 0.08), 0.5],
    accent: [mixHex(c.accent, c.mode === 'light' ? '#000000' : '#ffffff', 0.15), mid, c.accent2],
    menu: c.mode === 'light' ? mixHex(c.background, '#ffffff', 0.85) : mixHex(c.background, '#ffffff', 0.06),
    orb: c.mode === 'light' ? mixHex(c.background, '#ffffff', 0.7) : mixHex(c.background, '#ffffff', 0.04),
    trail: c.accent,
  };
}

// ── colour helpers ─────────────────────────────────────────────────────────

export function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  const f = h.length === 3 ? h.split('').map((x) => x + x).join('') : h.slice(0, 6);
  const n = parseInt(f, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgbToHex(r: number, g: number, b: number) {
  return '#' + [r, g, b].map((x) => Math.round(Math.max(0, Math.min(255, x))).toString(16).padStart(2, '0')).join('');
}

export function mixHex(a: string, b: string, t: number) {
  const x = hexToRgb(a), y = hexToRgb(b);
  return rgbToHex(x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t);
}

export function rgba(hex: string, a: number) {
  const [r, g, b] = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, +a.toFixed(3)))})`;
}

/** Scales saturation of a hex colour (1 = unchanged). */
export function saturate(hex: string, k: number) {
  if (k === 1) return hex;
  const [r, g, b] = hexToRgb(hex).map((v) => v / 255);
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  let h = 0, s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h /= 6;
  }
  s = Math.max(0, Math.min(1, s * k));
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s, p = 2 * l - q;
  const hue = (t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return s === 0 ? rgbToHex(l * 255, l * 255, l * 255) : rgbToHex(hue(h + 1 / 3) * 255, hue(h) * 255, hue(h - 1 / 3) * 255);
}
