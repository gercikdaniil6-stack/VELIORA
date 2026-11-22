import { useEffect, useRef } from 'react';

/**
 * The space behind the glass: a vertical wash and four soft light sources that
 * drift slowly (GPU transforms only), with a little parallax as pages scroll.
 * Colours and intensity come from the active theme tokens.
 */
export function Backdrop() {
  const backdrop = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = backdrop.current;
    if (!element) return;
    const delay = 8000;
    let last = 0;
    let timer = 0;
    const pause = () => {
      window.clearTimeout(timer);
      timer = 0;
      element.dataset.active = 'false';
    };
    const expire = () => {
      const remaining = delay - (performance.now() - last);
      if (remaining > 0) timer = window.setTimeout(expire, remaining);
      else pause();
    };
    const activate = () => {
      if (document.hidden || document.documentElement.dataset.bgAnim !== 'on') return;
      last = performance.now();
      if (timer) return;
      element.dataset.active = 'true';
      timer = window.setTimeout(expire, delay);
    };
    const visibility = () => { if (document.hidden) pause(); };
    const events = new AbortController();
    for (const event of ['pointermove', 'pointerdown', 'keydown', 'wheel']) {
      window.addEventListener(event, activate, { passive: true, signal: events.signal });
    }
    document.addEventListener('visibilitychange', visibility, { signal: events.signal });
    return () => { pause(); events.abort(); };
  }, []);
  return (
    <div className="backdrop" ref={backdrop} data-active="false" aria-hidden="true">
      <div className="backdrop__wash" />
      <div className="backdrop__light backdrop__light--1" />
      <div className="backdrop__light backdrop__light--2" />
      <div className="backdrop__light backdrop__light--3" />
      <div className="backdrop__light backdrop__light--4" />
      <div className="backdrop__vignette" />
      {/* Displacement maps for the refraction (lens) effect on round glass. */}
      <svg width="0" height="0" style={{ position: 'absolute' }}>
        <filter id="lens" x="0" y="0" width="100%" height="100%" colorInterpolationFilters="sRGB">
          <feImage href={LENS_MAP} x="0" y="0" width="100%" height="100%" preserveAspectRatio="none" result="map" />
          <feDisplacementMap in="SourceGraphic" in2="map" scale="38" xChannelSelector="R" yChannelSelector="G" />
        </filter>
      </svg>
    </div>
  );
}

// A radial displacement map: neutral (128) in the middle, bending outward toward the rim — like a glass lens.
const LENS_MAP =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256">
      <defs>
        <linearGradient id="x" x1="0" x2="1"><stop offset="0" stop-color="#ff0000"/><stop offset="0.5" stop-color="#800000"/><stop offset="1" stop-color="#000000"/></linearGradient>
        <linearGradient id="y" y1="0" y2="1" x1="0" x2="0"><stop offset="0" stop-color="#00ff00"/><stop offset="0.5" stop-color="#008000"/><stop offset="1" stop-color="#000000"/></linearGradient>
        <radialGradient id="m"><stop offset="0.55" stop-color="#fff"/><stop offset="1" stop-color="#000"/></radialGradient>
        <mask id="k"><rect width="256" height="256" fill="url(#m)"/></mask>
      </defs>
      <rect width="256" height="256" fill="url(#x)"/>
      <rect width="256" height="256" fill="url(#y)" style="mix-blend-mode:screen"/>
      <rect width="256" height="256" fill="#808000" mask="url(#k)"/>
    </svg>`,
  );
