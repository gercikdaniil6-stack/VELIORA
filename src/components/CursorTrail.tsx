import { lazy, Suspense, useEffect, useMemo, useState } from 'react';
import { computeVars, useActiveTheme } from '../theme/engine';
const SplashCursor = lazy(() => import('./SplashCursor/SplashCursor'));
const IDLE_MS = 25000;

/**
 * The app's single cursor-trail system: React Bits SplashCursor (WebGL2 fluid
 * ink). It is drawn BEHIND the interface — above the backdrop, below the shell —
 * so the glass panels blur it like real glass would. Pointer events pass
 * through; the simulation sleeps when the ink has faded and frees its GPU
 * context when switched off.
 */
export function CursorTrail() {
  const active = useActiveTheme();
  const appearance = active?.appearance;
  const theme = active?.theme;
  const enabled = appearance?.cursor.enabled ?? false;
  const [awake, setAwake] = useState(false);
  useEffect(() => {
    if (!enabled) return;
    let timer = 0;
    let last = 0;
    const sleep = () => {
      clearTimeout(timer);
      timer = 0;
      setAwake(false);
    };
    const check = () => {
      const remaining = IDLE_MS - (performance.now() - last);
      if (remaining > 0) timer = window.setTimeout(check, remaining);
      else sleep();
    };
    const wake = () => {
      if (document.hidden) return;
      last = performance.now();
      if (timer) return;
      setAwake(true);
      timer = window.setTimeout(check, IDLE_MS);
    };
    const visibility = () => { if (document.hidden) sleep(); };
    window.addEventListener('pointermove', wake, { passive: true });
    window.addEventListener('pointerdown', wake, { passive: true });
    document.addEventListener('visibilitychange', visibility);
    return () => {
      sleep();
      window.removeEventListener('pointermove', wake);
      window.removeEventListener('pointerdown', wake);
      document.removeEventListener('visibilitychange', visibility);
    };
  }, [enabled]);

  // Ink colours follow the active theme (after accent / saturation settings).
  const colors = useMemo(() => {
    if (!theme || !appearance) return undefined;
    const v = computeVars(theme, appearance);
    if (appearance.cursor.palette === 'accent') return [v['--accent-2']];
    return [v['--accent-1'], v['--accent-2'], v['--accent-3']];
  }, [theme, appearance]);

  if (!theme || !appearance || !enabled || !awake) return null;
  const c = appearance.cursor;
  const light = theme.mode === 'light';
  const calm = appearance.reducedMotion || !appearance.uiAnimation;
  return (
    <Suspense fallback={null}><SplashCursor
      className="cursor-ink"
      zIndex={0}
      theme={light ? 'light' : 'dark'}
      rainbow={c.palette === 'rainbow'}
      colors={colors}
      colorSpeed={c.palette === 'accent' ? 0 : 0.6}
      intensity={c.intensity * (light ? 0.5 : 1)}
      size={c.size}
      swirl={calm ? Math.min(c.swirl, 1) : c.swirl}
      trail={c.trail}
      glow={light ? 0 : c.glow * appearance.glow * 0.5}
      shading={light ? 0.6 : 1}
      clickSplash={c.splash && !calm}
      quality={calm || theme.id === 'obsidian' ? 'low' : 'medium'}
      intro={false}
    /></Suspense>
  );
}
