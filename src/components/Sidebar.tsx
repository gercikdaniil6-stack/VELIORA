import { useLayoutEffect, useRef, useState, type CSSProperties } from 'react';
import { useCore } from '../state/core';
import { Icon, type IconName } from './Icon';
import { StatusDot } from './bits';

export type Route = 'home' | 'telegram' | 'zapret' | 'lists' | 'catalog' | 'logs' | 'appearance' | 'settings' | 'about';

type Item = { id: Route; label: string; icon: IconName };
const MAIN: Item[] = [
  { id: 'home', label: 'Главная', icon: 'home' },
  { id: 'catalog', label: 'Каталог обходов', icon: 'layers' },
  { id: 'zapret', label: 'Zapret', icon: 'shield' },
  { id: 'telegram', label: 'Telegram', icon: 'telegram' },
];
const APP: Item[] = [
  { id: 'appearance', label: 'Оформление', icon: 'palette' },
  { id: 'logs', label: 'Журнал', icon: 'logs' },
  { id: 'settings', label: 'Настройки', icon: 'settings' },
];

const phase = (s: string) =>
  (s === 'running' ? 'connected' : s === 'starting' || s === 'stopping' ? 'connecting' : s === 'error' ? 'error' : 'idle') as 'connected' | 'connecting' | 'error' | 'idle';

export function Sidebar({ route, onNavigate }: { route: Route; onNavigate: (r: Route) => void }) {
  const { status, relay } = useCore();
  const nav = useRef<HTMLElement>(null);
  const [pill, setPill] = useState<{ y: number; h: number } | null>(null);

  // The glass pill glides to the active item.
  useLayoutEffect(() => {
    const measure = () => {
      const el = nav.current?.querySelector<HTMLElement>(`[data-route="${route}"]`);
      setPill(el ? { y: el.offsetTop, h: el.offsetHeight } : null);
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (nav.current) ro.observe(nav.current);
    return () => ro.disconnect();
  }, [route]);

  const item = (n: Item, i: number) => {
    const dot = n.id === 'telegram' ? phase(relay.state) : n.id === 'zapret' ? phase(status.zapret.state) : null;
    return (
      <button key={n.id} data-route={n.id} className="nav__item" style={{ '--i': i } as CSSProperties}
        aria-current={route === n.id ? 'page' : undefined} onClick={() => onNavigate(n.id)}>
        <Icon name={n.icon} />
        <span className="nav__text">{n.label}</span>
        {dot && dot !== 'idle' && <StatusDot phase={dot} />}
      </button>
    );
  };

  return (
    <nav className="sidebar" aria-label="Навигация" ref={nav}>
      {pill && <span className="nav__pill" style={{ transform: `translateY(${pill.y}px)`, height: pill.h }} aria-hidden="true" />}
      {MAIN.map((n, i) => item(n, i))}
      <div className="nav__label">Приложение</div>
      {APP.map((n, i) => item(n, i + MAIN.length))}
      <div className="sidebar__spacer" />
      <div className="mini-status glass">
        <div className="mini-status__row"><StatusDot phase={phase(relay.state)} /><span className="mini-status__name">Telegram</span><b>{relay.state === 'running' ? 'вкл' : 'выкл'}</b></div>
        <div className="mini-status__row"><StatusDot phase={phase(status.zapret.state)} /><span className="mini-status__name">Zapret</span><b>{status.zapret.state === 'running' ? 'вкл' : 'выкл'}</b></div>
      </div>
    </nav>
  );
}
