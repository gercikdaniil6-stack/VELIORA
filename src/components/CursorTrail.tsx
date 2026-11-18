import { useMemo } from 'react';
import { computeVars, useActiveTheme } from '../theme/engine';
import SplashCursor from './SplashCursor/SplashCursor';

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

  // Ink colours follow the active theme (after accent / saturation settings).
  const colors = useMemo(() => {
    if (!theme || !appearance) return undefined;
    const v = computeVars(theme, appearance);
    if (appearance.cursor.palette === 'accent') return [v['--accent-2']];
    return [v['--accent-1'], v['--accent-2'], v['--accent-3']];
  }, [theme, appearance]);

  if (!theme || !appearance || !appearance.cursor.enabled) return null;
  const c = appearance.cursor;
  const light = theme.mode === 'light';
  const calm = appearance.reducedMotion || !appearance.uiAnimation;
  return (
    <SplashCursor
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
      quality="medium"
      intro={false}
    />
  );
}
