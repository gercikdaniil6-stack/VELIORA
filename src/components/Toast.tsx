import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from 'react';
import { Icon } from './Icon';

export type Tone = 'info' | 'ok' | 'error' | 'busy';
export interface ToastData { tone?: Tone; title: string; body?: string; ms?: number }
interface Item extends ToastData { id: number; leaving?: boolean }

interface ToastApi {
  show: (t: ToastData) => number;
  update: (id: number, t: ToastData) => void;
  dismiss: (id: number) => void;
}

const Ctx = createContext<ToastApi | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Item[]>([]);
  const seq = useRef(1);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());

  const dismiss = useCallback((id: number) => {
    const t = timers.current.get(id);
    if (t) clearTimeout(t);
    timers.current.delete(id);
    // play the exit animation, then remove
    setItems((l) => l.map((x) => (x.id === id ? { ...x, leaving: true } : x)));
    setTimeout(() => setItems((l) => l.filter((x) => x.id !== id)), 240);
  }, []);

  const arm = useCallback((id: number, t: ToastData) => {
    const old = timers.current.get(id);
    if (old) clearTimeout(old);
    if (t.tone === 'busy') return;
    const ms = t.ms ?? (t.tone === 'error' ? 7000 : 3600);
    timers.current.set(id, setTimeout(() => dismiss(id), ms));
  }, [dismiss]);

  const show = useCallback((t: ToastData) => {
    const id = seq.current++;
    setItems((l) => [...l.slice(-3), { ...t, id }]);
    arm(id, t);
    return id;
  }, [arm]);

  const update = useCallback((id: number, t: ToastData) => {
    setItems((l) => (l.some((x) => x.id === id) ? l.map((x) => (x.id === id ? { ...t, id } : x)) : [...l, { ...t, id }]));
    arm(id, t);
  }, [arm]);

  const api = useMemo(() => ({ show, update, dismiss }), [show, update, dismiss]);

  return (
    <Ctx.Provider value={api}>
      {children}
      <div className="toasts" role="status" aria-live="polite">
        {items.map((t) => (
          <div key={t.id} className={`toast glass glass--strong toast--${t.tone ?? 'info'}${t.leaving ? ' is-leaving' : ''}`}>
            <span className="toast__icon">
              {t.tone === 'busy' ? <span className="spinner" /> : <Icon name={t.tone === 'ok' ? 'check' : t.tone === 'error' ? 'alert' : 'info'} size={16} />}
            </span>
            <div className="toast__text">
              <div className="toast__title">{t.title}</div>
              {t.body && <div className="toast__body">{t.body}</div>}
            </div>
            <button className="toast__close" aria-label="Скрыть" onClick={() => dismiss(t.id)}><Icon name="close" size={14} /></button>
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

export function useToast() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useToast outside ToastProvider');
  return v;
}
