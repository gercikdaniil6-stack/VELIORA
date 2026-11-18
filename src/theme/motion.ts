import { useEffect, type RefObject } from 'react';

const GLOW_TARGETS = '.glass, .tile, .strategy, .ipset__opt, .credit, .theme-card, .btn';

/**
 * One window-level listener for the light that follows the cursor across glass:
 * writes --mx/--my on the surface under the pointer, at most once per frame.
 */
export function useGlassPointer() {
  useEffect(() => {
    let raf = 0;
    let last: PointerEvent | null = null;
    let lit: HTMLElement | null = null;
    const frame = () => {
      raf = 0;
      if (!last) return;
      const el = (last.target as Element | null)?.closest?.(GLOW_TARGETS) as HTMLElement | null;
      if (lit && lit !== el) lit.classList.remove('is-lit');
      lit = el;
      if (!el) return;
      const r = el.getBoundingClientRect();
      el.style.setProperty('--mx', `${last.clientX - r.left}px`);
      el.style.setProperty('--my', `${last.clientY - r.top}px`);
      el.classList.add('is-lit');
    };
    const move = (e: PointerEvent) => {
      last = e;
      if (!raf) raf = requestAnimationFrame(frame);
    };
    const leave = () => { lit?.classList.remove('is-lit'); lit = null; };
    window.addEventListener('pointermove', move, { passive: true });
    document.documentElement.addEventListener('pointerleave', leave);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener('pointermove', move);
      document.documentElement.removeEventListener('pointerleave', leave);
    };
  }, []);
}

/**
 * Cards that start below the fold rise in as they enter the view (once).
 * Cards already visible on arrival use the page's own staggered entrance.
 */
export function useReveal(scroller: RefObject<HTMLElement | null>, key: string) {
  useEffect(() => {
    const root = scroller.current;
    if (!root) return;
    let io: IntersectionObserver | null = null;
    const t = setTimeout(() => {
      const view = root.getBoundingClientRect();
      const items = [...root.querySelectorAll<HTMLElement>('.page .glass, .page .reveal')].filter((el) => !el.closest('.sheet'));
      io = new IntersectionObserver(
        (entries) => {
          for (const e of entries) {
            if (e.isIntersecting) {
              e.target.classList.add('is-in');
              io?.unobserve(e.target);
            }
          }
        },
        { root, rootMargin: '0px 0px -6% 0px', threshold: 0.04 },
      );
      for (const el of items) {
        if (el.getBoundingClientRect().top > view.bottom - 8) {
          el.classList.add('will-reveal');
          io.observe(el);
        }
      }
    }, 60);
    return () => { clearTimeout(t); io?.disconnect(); };
  }, [scroller, key]);
}

/** Exposes the page scroll as --scroll (for gentle parallax), throttled to frames. */
export function useScrollVar(scroller: RefObject<HTMLElement | null>, key: string) {
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    let raf = 0;
    const root = document.documentElement;
    const write = () => { raf = 0; root.style.setProperty('--scroll', String(Math.round(el.scrollTop))); };
    const on = () => { if (!raf) raf = requestAnimationFrame(write); };
    write();
    el.addEventListener('scroll', on, { passive: true });
    return () => { el.removeEventListener('scroll', on); cancelAnimationFrame(raf); root.style.setProperty('--scroll', '0'); };
  }, [scroller, key]);
}
