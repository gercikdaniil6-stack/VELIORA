import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { CoreState, CoreStatus } from '../lib/api';
import { Icon, type IconName } from './Icon';

export const STATE_TEXT: Record<CoreState, string> = {
  stopped: 'Выключен',
  starting: 'Запускается',
  running: 'Включён',
  stopping: 'Останавливается',
  error: 'Ошибка',
};

/** Keeps a closing element mounted for its exit animation. */
export function usePresence(open: boolean, ms = 220) {
  const [mounted, setMounted] = useState(open);
  const [leaving, setLeaving] = useState(false);
  useEffect(() => {
    if (open) { setMounted(true); setLeaving(false); return; }
    if (!mounted) return;
    setLeaving(true);
    const t = setTimeout(() => { setMounted(false); setLeaving(false); }, ms);
    return () => clearTimeout(t);
  }, [open, ms, mounted]);
  return { mounted, leaving };
}

export function Modal({ open, onClose, title, lead, children, foot, wide }: {
  open: boolean; onClose: () => void; title: string; lead?: ReactNode; children?: ReactNode; foot?: ReactNode; wide?: boolean;
}) {
  const { mounted, leaving } = usePresence(open);
  useEffect(() => {
    if (!open) return;
    const k = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', k);
    return () => window.removeEventListener('keydown', k);
  }, [open, onClose]);
  if (!mounted) return null;
  return createPortal(
    <div className={`scrim${leaving ? ' is-leaving' : ''}`} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`sheet glass glass--strong${wide ? ' sheet--wide' : ''}`} role="dialog" aria-modal="true" aria-label={title}>
        <h2>{title}</h2>
        {lead && <p className="muted" style={{ margin: 0 }}>{lead}</p>}
        {children && <div className="sheet__body">{children}</div>}
        {foot && <div className="sheet__foot">{foot}</div>}
      </div>
    </div>,
    document.body,
  );
}

export function Confirm({ open, title, body, confirm, danger, onConfirm, onClose }: {
  open: boolean; title: string; body: ReactNode; confirm: string; danger?: boolean; onConfirm: () => void; onClose: () => void;
}) {
  return (
    <Modal open={open} onClose={onClose} title={title} lead={body}
      foot={<>
        <button className="btn btn--ghost" onClick={onClose}>Отмена</button>
        <button className={`btn ${danger ? 'btn--danger-solid' : 'btn--primary'}`} onClick={() => { onConfirm(); onClose(); }}>{confirm}</button>
      </>} />
  );
}

export function Banner({ tone = 'info', icon, title, children, actions }: {
  tone?: 'info' | 'warn' | 'error' | 'accent'; icon?: IconName; title: ReactNode; children?: ReactNode; actions?: ReactNode;
}) {
  return (
    <div className={`banner glass banner--${tone}`}>
      <span className="banner__icon"><Icon name={icon ?? (tone === 'error' || tone === 'warn' ? 'alert' : 'sparkle')} size={18} /></span>
      <div className="banner__text">
        <div className="banner__title">{title}</div>
        {children && <div className="banner__body">{children}</div>}
      </div>
      {actions && <div className="banner__actions">{actions}</div>}
    </div>
  );
}

/** Segmented control with a glass thumb that slides to the chosen option. */
export function Seg<T extends string>({ value, options, onChange, disabled, label }: {
  value: T; options: { value: T; label: string; icon?: IconName }[]; onChange: (v: T) => void; disabled?: boolean; label: string;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [thumb, setThumb] = useState<{ x: number; w: number } | null>(null);
  useLayoutEffect(() => {
    const measure = () => {
      const el = box.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]');
      if (el) setThumb({ x: el.offsetLeft, w: el.offsetWidth });
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (box.current) ro.observe(box.current);
    return () => ro.disconnect();
  }, [value, options.length]);
  return (
    <div className="segmented" role="group" aria-label={label} ref={box}>
      {thumb && <span className="segmented__thumb" style={{ transform: `translateX(${thumb.x - 3}px)`, width: thumb.w }} aria-hidden="true" />}
      {options.map((o) => (
        <button key={o.value} type="button" aria-pressed={value === o.value} disabled={disabled} onClick={() => onChange(o.value)}>
          {o.icon && <Icon name={o.icon} size={15} />}{o.label}
        </button>
      ))}
    </div>
  );
}

/** Dropdown with an animated glass menu; keyboard: arrows, Enter, Esc. */
export function SelectBox({ value, onChange, options, disabled, label }: {
  value: string; onChange: (v: string) => void; options: { value: string; label: string; detail?: string; fresh?: boolean }[]; disabled?: boolean; label: string;
}) {
  const [open, setOpen] = useState(false);
  const [hi, setHi] = useState(0);
  const wrap = useRef<HTMLDivElement>(null);
  const { mounted, leaving } = usePresence(open, 160);
  const current = options.find((o) => o.value === value) ?? options[0];
  useEffect(() => {
    if (!open) return;
    setHi(Math.max(0, options.findIndex((o) => o.value === value)));
    const away = (e: MouseEvent) => { if (!wrap.current?.contains(e.target as Node)) setOpen(false); };
    window.addEventListener('mousedown', away);
    return () => window.removeEventListener('mousedown', away);
  }, [open, options, value]);
  const pick = (v: string) => { onChange(v); setOpen(false); };
  const key = (e: React.KeyboardEvent) => {
    if (disabled) return;
    if (!open && (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown')) { e.preventDefault(); setOpen(true); return; }
    if (!open) return;
    if (e.key === 'Escape') { e.preventDefault(); setOpen(false); }
    if (e.key === 'ArrowDown') { e.preventDefault(); setHi((h) => Math.min(options.length - 1, h + 1)); }
    if (e.key === 'ArrowUp') { e.preventDefault(); setHi((h) => Math.max(0, h - 1)); }
    if (e.key === 'Enter' && options[hi]) { e.preventDefault(); pick(options[hi].value); }
  };
  return (
    <div className={`select${open ? ' is-open' : ''}`} ref={wrap}>
      <button type="button" className="select__button" aria-haspopup="listbox" aria-expanded={open} aria-label={label} disabled={disabled}
        onClick={() => setOpen(!open)} onKeyDown={key}>
        <span className="select__value">{current?.label}</span>
        {current?.fresh && <span className="manager-new">NEW</span>}
        <Icon name="chevronDown" size={16} />
      </button>
      {mounted && (
        <div className={`select__menu glass glass--strong${leaving ? ' is-leaving' : ''}`} role="listbox" aria-label={label}>
          {options.map((o, i) => (
            <div key={o.value} role="option" aria-selected={o.value === value} className={`select__option${i === hi ? ' is-hi' : ''}`}
              onMouseEnter={() => setHi(i)} onMouseDown={(e) => { e.preventDefault(); pick(o.value); }}>
              <span>{o.label}{o.detail && <small className="manager-version-detail">{o.detail}</small>}</span>
              {o.fresh && <span className="manager-new">NEW</span>}
              {o.value === value && <Icon name="check" size={14} />}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Range slider with a filled track and a live value. */
export function Slider({ label, value, min, max, step = 0.05, onChange, format, hint }: {
  label: string; value: number; min: number; max: number; step?: number; onChange: (v: number) => void; format?: (v: number) => string; hint?: string;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <label className="slider">
      <span className="slider__head">
        <span className="slider__label">{label}</span>
        <span className="slider__value mono">{format ? format(value) : `${Math.round(value * 100)}%`}</span>
      </span>
      <input type="range" min={min} max={max} step={step} value={value} style={{ '--pct': `${pct}%` } as React.CSSProperties}
        onChange={(e) => onChange(+e.target.value)} aria-label={label} />
      {hint && <span className="field__hint">{hint}</span>}
    </label>
  );
}

export function Skeleton({ rows = 4, height = 46 }: { rows?: number; height?: number }) {
  return (
    <div className="skeleton" aria-busy="true" aria-label="Загрузка">
      {Array.from({ length: rows }, (_, i) => <span key={i} className="skeleton__row" style={{ height, animationDelay: `${i * 90}ms` }} />)}
    </div>
  );
}

export const Spinner = ({ size = 16 }: { size?: number }) => <span className="spinner" style={{ width: size, height: size }} aria-hidden="true" />;

/** Live uptime "hh:mm:ss" for a running component. */
export function useUptime(st: CoreStatus) {
  const [now, setNow] = useState(Date.now());
  const running = st.state === 'running' && !!st.startedAt;
  useEffect(() => {
    if (!running) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [running]);
  if (!running) return '00:00:00';
  const s = Math.max(0, Math.floor((now - (st.startedAt as number)) / 1000));
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(Math.floor(s / 3600))}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`;
}

export function ServiceCard({ icon, title, subtitle, status, pending, onToggle, version, disabled, disabledReason, children }: {
  icon: IconName; title: string; subtitle?: ReactNode; status: CoreStatus; pending: boolean; onToggle: (on: boolean) => void;
  version?: string; disabled?: boolean; disabledReason?: string; children?: ReactNode;
}) {
  const on = status.state === 'running';
  const transitioning = pending || status.state === 'starting' || status.state === 'stopping';
  const uptime = useUptime(status);
  return (
    <section className={`service glass service--${status.state}`}>
      <div className="service__top">
        <span className="service__icon"><Icon name={icon} size={22} /></span>
        <div className="service__head">
          <div className="service__title">{title}</div>
          {subtitle && <div className="service__sub">{subtitle}</div>}
        </div>
        <div className="service__toggle">
          <button type="button" role="switch" aria-checked={on} aria-label={`${title}: ${on ? 'выключить' : 'включить'}`} className="switch switch--lg"
            disabled={transitioning || disabled} title={disabled ? disabledReason : undefined} onClick={() => onToggle(!on)} />
          {version && <span className="service__ver">v{version}</span>}
        </div>
      </div>
      <div className="service__foot">
        <span className="service__state">{transitioning && <Spinner size={12} />}{STATE_TEXT[status.state]}{on && <span className="mono service__uptime">{uptime}</span>}</span>
        {disabled && disabledReason ? <span className="service__note">{disabledReason}</span> : children}
      </div>
      {status.state === 'error' && status.lastError && <div className="service__error">{status.lastError}</div>}
    </section>
  );
}

export function Field({ label, hint, error, children }: { label: string; hint?: ReactNode; error?: string | null; children: ReactNode }) {
  return (
    <label className="field">
      <span className="field__label">{label}</span>
      {children}
      {error ? <span className="field__error">{error}</span> : hint ? <span className="field__hint">{hint}</span> : null}
    </label>
  );
}

export function Help({ text }: { text: string }) {
  return <span className="help" tabIndex={0} aria-label={text} data-tip={text}>?</span>;
}
