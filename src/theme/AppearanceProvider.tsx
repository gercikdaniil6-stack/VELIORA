import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { isTauri } from '../lib/native';
import { useCore } from '../state/core';
import { apply, DEFAULT_APPEARANCE, loadLocal, normalize, onSystemChange, saveLocal, type Appearance } from './engine';

interface Ctx {
  appearance: Appearance;
  /** Applies instantly; persisted (debounced) when "Сохранять настройки" is on. */
  update: (patch: Partial<Appearance> | ((a: Appearance) => Appearance)) => void;
  reset: () => void;
}

const AppearanceCtx = createContext<Ctx | null>(null);

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const core = useCore();
  const [appearance, setAppearance] = useState<Appearance>(loadLocal);
  const ref = useRef(appearance);
  const adopted = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // config.json is the durable copy: adopt it once it has loaded (e.g. after a WebView data reset).
  useEffect(() => {
    if (!core.loaded || adopted.current) return;
    adopted.current = true;
    if (core.config.appearance) {
      const remote = normalize(core.config.appearance as Partial<Appearance>);
      if (JSON.stringify(remote) !== JSON.stringify(ref.current)) {
        ref.current = remote;
        setAppearance(remote);
        saveLocal(remote);
        apply(remote, true);
      }
    }
  }, [core.loaded, core.config.appearance]);

  const persist = useCallback((a: Appearance) => {
    saveLocal(a);
    clearTimeout(timer.current);
    if (!isTauri) return; // browser preview: localStorage only
    timer.current = setTimeout(() => {
      core.save((c) => ({ ...c, appearance: a }), { quiet: true });
    }, 450);
  }, [core]);

  const update = useCallback<Ctx['update']>((patch) => {
    const prev = ref.current;
    const next = normalize(typeof patch === 'function' ? patch(prev) : { ...prev, ...patch, cursor: { ...prev.cursor, ...(patch.cursor ?? {}) } });
    ref.current = next;
    setAppearance(next);
    apply(next, next.theme !== prev.theme || next.custom.mode !== prev.custom.mode);
    // When saving is off, only the switch itself is remembered.
    if (next.persist) persist(next);
    else if (prev.persist) persist({ ...prev, persist: false });
  }, [persist]);

  const reset = useCallback(() => update({ ...DEFAULT_APPEARANCE, persist: ref.current.persist }), [update]);

  // "System" follows Windows' light/dark setting live.
  useEffect(() => onSystemChange(() => { if (ref.current.theme === 'system') apply(ref.current, true); }), []);

  // Pause decorative animation while the window is hidden (tray) or minimised.
  useEffect(() => {
    const sync = () => { document.documentElement.dataset.hidden = document.visibilityState === 'hidden' ? 'true' : 'false'; };
    sync();
    document.addEventListener('visibilitychange', sync);
    return () => document.removeEventListener('visibilitychange', sync);
  }, []);

  return <AppearanceCtx.Provider value={{ appearance, update, reset }}>{children}</AppearanceCtx.Provider>;
}

export function useAppearance() {
  const v = useContext(AppearanceCtx);
  if (!v) throw new Error('useAppearance outside AppearanceProvider');
  return v;
}
