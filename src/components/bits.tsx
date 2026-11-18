import type { Latency, Phase } from '../state/types';

export function phaseTitle(p: Phase) {
  return {
    idle: 'Не подключено',
    connecting: 'Подключение…',
    connected: 'Подключено',
    disconnecting: 'Отключение…',
    error: 'Ошибка подключения',
  }[p];
}

export function StatusDot({ phase }: { phase: Phase }) {
  const cls =
    phase === 'connected' ? 'dot dot--on'
      : phase === 'connecting' || phase === 'disconnecting' ? 'dot dot--busy'
        : phase === 'error' ? 'dot dot--err' : 'dot';
  return <span className={cls} aria-hidden="true" />;
}

export function LatencyBadge({ v }: { v?: Latency[string] }) {
  if (!v || (v.ms == null && !v.error && !v.pending)) return <span className="latency latency--none">—</span>;
  if (v.pending && v.ms == null) return <span className="latency latency--none">проверка…</span>;
  if (v.error) return <span className="latency latency--err" title={v.error}>нет ответа</span>;
  const ms = v.ms!;
  const cls = ms < 80 ? 'good' : ms < 200 ? 'mid' : 'bad';
  return <span className={`latency latency--${cls}`}>{formatMs(ms)}</span>;
}

export function Switch({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return <button type="button" role="switch" aria-checked={checked} aria-label={label} className="switch" disabled={disabled} onClick={() => onChange(!checked)} />;
}

export function Setting({ title, desc, checked, onChange, disabled }: { title: string; desc?: string; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <div className={`setting${disabled ? ' setting--disabled' : ''}`}>
      <div>
        <div className="setting__title">{title}</div>
        {desc && <div className="setting__desc">{desc}</div>}
      </div>
      <Switch checked={checked} onChange={onChange} label={title} disabled={disabled} />
    </div>
  );
}

export function formatDuration(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return h ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

export const formatMs = (ms: number) => (ms < 1 ? '<1 мс' : `${ms} мс`);
