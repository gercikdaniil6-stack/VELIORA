// Theme engine: turns (theme × appearance settings) into CSS custom properties
// on :root. Every component reads those tokens, so one apply() restyles the
// whole app without re-rendering React.
import { useSyncExternalStore } from 'react';
import { customTheme, DEFAULT_CUSTOM, mixHex, rgba, saturate, THEME_BY_ID, type CustomTheme, type Mode, type ThemeDef } from './themes';

export type TrailPalette = 'theme' | 'rainbow' | 'accent';

export interface Appearance {
  /** theme id, 'system' or 'custom' */
  theme: string;
  custom: CustomTheme;
  /** Accent override (null = theme's own) */
  accent: string | null;
  blur: number; // 0–2 × theme blur
  opacity: number; // 0.4–1.6 × panel opacity
  glow: number; // 0–2
  gradient: number; // 0–2 background light
  radius: number; // 0.5–1.6
  shadow: number; // 0–2
  speed: number; // 0.5–2 (higher = faster)
  saturation: number; // 0.4–1.6
  glass: number; // 0–2 reflections & highlights
  liquidGlass: boolean;
  refraction: boolean;
  bgAnimation: boolean;
  uiAnimation: boolean;
  reducedMotion: boolean;
  /** Fluid ink trail (SplashCursor) drawn behind the glass */
  cursor: { enabled: boolean; palette: TrailPalette; intensity: number; size: number; trail: number; swirl: number; glow: number; splash: boolean };
  persist: boolean;
}

export const DEFAULT_APPEARANCE: Appearance = {
  theme: 'liquid-dark',
  custom: DEFAULT_CUSTOM,
  accent: null,
  blur: 1,
  opacity: 1,
  glow: 1,
  gradient: 1,
  radius: 1,
  shadow: 1,
  speed: 1,
  saturation: 1,
  glass: 1,
  liquidGlass: true,
  refraction: true,
  bgAnimation: true,
  uiAnimation: true,
  reducedMotion: false,
  cursor: { enabled: true, palette: 'theme', intensity: 0.6, size: 1, trail: 1, swirl: 3, glow: 0.2, splash: true },
  persist: true,
};

export function normalize(a: Partial<Appearance> | null | undefined): Appearance {
  const d = DEFAULT_APPEARANCE;
  const n = (v: unknown, lo: number, hi: number, def: number) => (typeof v === 'number' && isFinite(v) ? Math.min(hi, Math.max(lo, v)) : def);
  const src = a ?? {};
  const c = (src.cursor ?? {}) as Partial<Appearance['cursor']>;
  const valid = (id: unknown) => typeof id === 'string' && (id === 'system' || id === 'custom' || !!THEME_BY_ID[id]);
  const hex = (v: unknown, def: string) => (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v) ? v : def);
  const cu = (src.custom ?? {}) as Partial<CustomTheme>;
  return {
    theme: valid(src.theme) ? (src.theme as string) : d.theme,
    custom: { mode: cu.mode === 'light' ? 'light' : 'dark', accent: hex(cu.accent, d.custom.accent), accent2: hex(cu.accent2, d.custom.accent2), background: hex(cu.background, d.custom.background) },
    accent: typeof src.accent === 'string' && /^#[0-9a-f]{6}$/i.test(src.accent) ? src.accent : null,
    blur: n(src.blur, 0, 2, 1),
    opacity: n(src.opacity, 0.4, 1.6, 1),
    glow: n(src.glow, 0, 2, 1),
    gradient: n(src.gradient, 0, 2, 1),
    radius: n(src.radius, 0.5, 1.6, 1),
    shadow: n(src.shadow, 0, 2, 1),
    speed: n(src.speed, 0.5, 2, 1),
    saturation: n(src.saturation, 0.4, 1.6, 1),
    glass: n(src.glass, 0, 2, 1),
    liquidGlass: src.liquidGlass ?? d.liquidGlass,
    refraction: src.refraction ?? d.refraction,
    bgAnimation: src.bgAnimation ?? d.bgAnimation,
    uiAnimation: src.uiAnimation ?? d.uiAnimation,
    reducedMotion: src.reducedMotion ?? d.reducedMotion,
    cursor: {
      enabled: c.enabled ?? d.cursor.enabled,
      palette: (['theme', 'rainbow', 'accent'] as const).includes(c.palette as TrailPalette) ? (c.palette as TrailPalette) : d.cursor.palette,
      intensity: n(c.intensity, 0.1, 2, d.cursor.intensity),
      size: n(c.size, 0.3, 2.5, d.cursor.size),
      trail: n(c.trail, 0.3, 3, d.cursor.trail),
      swirl: n(c.swirl, 0, 8, d.cursor.swirl),
      glow: n(c.glow, 0, 1, d.cursor.glow),
      splash: typeof c.splash === 'boolean' ? c.splash : d.cursor.splash,
    },
    persist: src.persist ?? d.persist,
  };
}

// ── system theme ───────────────────────────────────────────────────────────

const darkQuery = typeof window !== 'undefined' ? window.matchMedia('(prefers-color-scheme: dark)') : null;
export const systemMode = (): Mode => (darkQuery && !darkQuery.matches ? 'light' : 'dark');

export function resolveTheme(a: Appearance): ThemeDef {
  if (a.theme === 'custom') return customTheme(a.custom);
  if (a.theme === 'system') return THEME_BY_ID[systemMode() === 'light' ? 'liquid-light' : 'liquid-dark'];
  return THEME_BY_ID[a.theme] ?? THEME_BY_ID['liquid-dark'];
}

// ── tokens ─────────────────────────────────────────────────────────────────

export function computeVars(t: ThemeDef, a: Appearance): Record<string, string> {
  const light = t.mode === 'light';
  const sat = (h: string) => saturate(h, a.saturation);
  const accent = a.accent
    ? [sat(mixHex(a.accent, light ? '#000000' : '#ffffff', light ? 0.06 : 0.22)), sat(a.accent), sat(mixHex(a.accent, t.accent[2], 0.45))]
    : t.accent.map(sat);
  const g = a.liquidGlass ? a.glass : 0;
  const spec = t.specular * g;
  const surfaceA = a.liquidGlass ? Math.min(0.97, Math.max(0.05, t.surface[1] * a.opacity)) : 0.97;
  const blur = a.liquidGlass ? Math.round(t.blur * a.blur) : 0;
  const sh = t.shade;
  const k = a.shadow;
  const r = (px: number) => `${Math.round(px * a.radius)}px`;
  const ms = (v: number) => `${Math.round(v / a.speed)}ms`;

  const vars: Record<string, string> = {
    '--bg': t.bg,
    '--bg-a': t.wash[0],
    '--bg-b': t.wash[1],
    '--blob-1': rgba(sat(t.blobs[0][0]), t.blobs[0][1] * a.gradient),
    '--blob-2': rgba(sat(t.blobs[1][0]), t.blobs[1][1] * a.gradient),
    '--blob-3': rgba(sat(t.blobs[2][0]), t.blobs[2][1] * a.gradient),
    '--blob-4': rgba(sat(t.blobs[3][0]), t.blobs[3][1] * a.gradient),
    '--vignette': light ? 'rgba(255, 255, 255, 0)' : rgba(sh, 0.5),
    '--ink': t.ink,
    '--fill': '#ffffff',
    '--fill-k': String(t.fillK),
    '--spec': '#ffffff',
    '--spec-2': light ? '#eef2f8' : '#dfe6f3',
    '--well': t.well,
    '--well-k': String(t.wellK),
    '--shade': sh,
    '--text-1': t.text[0],
    '--text-2': t.text[1],
    '--text-3': t.text[2],
    '--text-on-accent': t.onAccent,
    '--code': t.code,
    '--accent-1': accent[0],
    '--accent-2': accent[1],
    '--accent-3': accent[2],
    '--ok': t.ok,
    '--warn': t.warn,
    '--bad': t.bad,
    '--bad-text': light ? mixHex(t.bad, '#000000', 0.25) : mixHex(t.bad, '#ffffff', 0.55),
    '--on-bad': light ? '#ffffff' : '#2a0f16',
    '--surface': rgba(t.surface[0], surfaceA),
    '--surface-solid': t.menu,
    '--glass-fill': `linear-gradient(180deg, ${rgba('#ffffff', (light ? 0.34 : 0.066) * g)} 0%, ${rgba('#ffffff', (light ? 0.1 : 0.026) * g)} 100%)`,
    '--glass-fill-strong': `linear-gradient(180deg, ${rgba('#ffffff', (light ? 0.5 : 0.1) * g)} 0%, ${rgba('#ffffff', (light ? 0.18 : 0.045) * g)} 100%)`,
    '--glass-border': light ? rgba('#ffffff', 0.55 + 0.25 * Math.min(1, g)) : rgba(t.ink, 0.085),
    '--glass-border-strong': light ? rgba('#ffffff', 0.9) : rgba(t.ink, 0.14),
    '--glass-highlight': `inset 0 1px 0 ${rgba('#ffffff', Math.min(1, (light ? 0.85 : 0.09) * spec))}`,
    '--rim': rgba('#ffffff', Math.min(1, (light ? 0.9 : 0.18) * spec)),
    '--blur': `${blur}px`,
    '--glass-blur': blur ? `blur(${blur}px) saturate(${light ? 170 : 150}%)` : 'none',
    '--menu-bg': t.menu,
    '--orb-bg': t.orb,
    '--titlebar-bg': light ? `linear-gradient(180deg, ${rgba('#ffffff', 0.55)}, ${rgba('#ffffff', 0.2)})` : `linear-gradient(180deg, ${rgba(t.bg, 0.6)}, ${rgba(t.bg, 0.2)})`,
    '--sidebar-bg': light ? `linear-gradient(180deg, ${rgba('#ffffff', 0.42)}, ${rgba('#ffffff', 0.18)})` : `linear-gradient(180deg, ${rgba(t.wash[0], 0.4)}, ${rgba(t.bg, 0.15)})`,
    '--scrim': light ? rgba(mixHex(t.bg, '#9aa8c4', 0.4), 0.42) : rgba(sh, 0.55),
    '--shadow-1': light
      ? `0 1px 2px ${rgba(sh, 0.07 * k)}, 0 10px 30px ${rgba(sh, 0.1 * k)}`
      : `0 1px 2px ${rgba(sh, 0.3 * k)}, 0 8px 24px ${rgba(sh, 0.22 * k)}`,
    '--shadow-2': light
      ? `0 2px 6px ${rgba(sh, 0.08 * k)}, 0 28px 70px ${rgba(sh, 0.16 * k)}`
      : `0 2px 6px ${rgba(sh, 0.35 * k)}, 0 24px 60px ${rgba(sh, 0.45 * k)}`,
    '--glow-k': String(a.glow),
    '--r-xs': r(8),
    '--r-sm': r(12),
    '--r-md': r(16),
    '--r-lg': r(22),
    '--r-xl': r(28),
    '--t-fast': ms(140),
    '--t-med': ms(260),
    '--t-slow': ms(520),
    '--motion-k': String(1 / a.speed),
  };
  return vars;
}

function motionState(a: Appearance) {
  if (!a.uiAnimation) return 'off';
  if (a.reducedMotion) return 'reduced';
  return 'full';
}

let current: { theme: ThemeDef; appearance: Appearance } | null = null;
const listeners = new Set<() => void>();

/** Writes the tokens to :root. `animate` crossfades the whole window (View Transitions). */
export function apply(a: Appearance, animate = false) {
  const theme = resolveTheme(a);
  const root = document.documentElement;
  const write = () => {
    const vars = computeVars(theme, a);
    for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
    root.dataset.mode = theme.mode;
    root.dataset.theme = theme.id;
    root.dataset.glass = a.liquidGlass ? 'on' : 'off';
    root.dataset.motion = motionState(a);
    root.dataset.bgAnim = a.bgAnimation && !a.reducedMotion && a.uiAnimation ? 'on' : 'off';
    root.dataset.refraction = a.refraction && a.liquidGlass ? 'on' : 'off';
    root.setAttribute('data-motion-override', '');
    current = { theme, appearance: a };
    listeners.forEach((l) => l());
  };
  const prevMode = root.dataset.theme;
  const doc = document as Document & { startViewTransition?: (cb: () => void) => unknown };
  if (animate && prevMode !== theme.id && doc.startViewTransition && a.uiAnimation && !a.reducedMotion) doc.startViewTransition(write);
  else write();
  syncWindow(theme);
}

let lastBg = '';
async function syncWindow(t: ThemeDef) {
  if (!('__TAURI_INTERNALS__' in window) || lastBg === t.bg) return;
  lastBg = t.bg;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(t.bg.slice(i, i + 2), 16));
    await getCurrentWindow().setBackgroundColor([r, g, b, 255]);
  } catch {
    /* older runtime: the CSS background still covers the window */
  }
}

export function onSystemChange(cb: () => void) {
  darkQuery?.addEventListener('change', cb);
  return () => darkQuery?.removeEventListener('change', cb);
}

/** Current resolved theme (mode, trail colour…) for components outside the tokens. */
export function useActiveTheme() {
  return useSyncExternalStore(
    (cb) => { listeners.add(cb); return () => listeners.delete(cb); },
    () => current,
  );
}

export function useThemeMode(): Mode {
  return useActiveTheme()?.theme.mode ?? 'dark';
}

// ── storage: localStorage for an instant first paint, config.json for durability

const KEY = 'veliora.appearance.v1';

export function loadLocal(): Appearance {
  try {
    const raw = localStorage.getItem(KEY);
    return normalize(raw ? JSON.parse(raw) : null);
  } catch {
    return normalize(null);
  }
}

export function saveLocal(a: Appearance) {
  try { localStorage.setItem(KEY, JSON.stringify(a)); } catch { /* storage unavailable */ }
}
