import { useLayoutEffect, useRef, useState, type CSSProperties } from 'react';
import { useCore } from '../state/core';
import { useStore } from '../state/store';
import { Icon, type IconName } from './Icon';
import { StatusDot } from './bits';

export type Route = 'home' | 'telegram' | 'zapret' | 'lists' | 'proxy' | 'servers' | 'rules' | 'logs' | 'appearance' | 'settings' | 'about';

type Item = { id: Route; label: string; icon: IconName };
const MAIN: Item[] = [
  { id: 'home', label: 'Главная', icon: 'home' },
  { id: 'telegram', label: 'Telegram', icon: 'telegram' },
  { id: 'zapret', label: 'Zapret', icon: 'shield' },
  { id: 'lists', label: 'Списки', icon: 'lists' },
];
const PROXY: Item[] = [
  { id: 'proxy', label: 'Обзор', icon: 'globe' },
  { id: 'servers', label: 'Серверы', icon: 'servers' },
  { id: 'rules', label: 'Правила', icon: 'rules' },
];
const APP: Item[] = [
  { id: 'logs', label: 'Журнал', icon: 'logs' },
  { id: 'appearance', label: 'Оформление', icon: 'palette' },
  { id: 'settings', label: 'Настройки', icon: 'settings' },
  { id: 'about', label: 'О приложении', icon: 'about' },
];

const phase = (s: string) =>
  (s === 'running' ? 'connected' : s === 'starting' || s === 'stopping' ? 'connecting' : s === 'error' ? 'error' : 'idle') as 'connected' | 'connecting' | 'error' | 'idle';

export function Sidebar({ route, onNavigate }: { route: Route; onNavigate: (r: Route) => void }) {
  const { status, logs } = useCore();
  const { conn } = useStore();
  const problems = logs.filter((l) => l.level === 'error').length;
  const nav = useRef<HTMLElement>(null);
  const [pill, setPill] = useState<{ y: number; h: number } | null>(null);

  // The glass pill glides to the active item.
  useLayoutEffect(() => {
    const measure = () => {
      const el = nav.current?.querySelector<HTMLElement>(`[data-route="${route}"]`);
      if (el) setPill({ y: el.offsetTop, h: el.offsetHeight });
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (nav.current) ro.observe(nav.current);
    return () => ro.disconnect();
  }, [route]);

  const item = (n: Item, i: number) => {
    const dot = n.id === 'telegram' ? phase(status.tgws.state) : n.id === 'zapret' ? phase(status.zapret.state) : n.id === 'proxy' ? conn.phase : null;
    return (
      <button key={n.id} data-route={n.id} className="nav__item" style={{ '--i': i } as CSSProperties}
        aria-current={route === n.id ? 'page' : undefined} onClick={() => onNavigate(n.id)}>
        <Icon name={n.icon} />
        <span className="nav__text">{n.label}</span>
        {dot && dot !== 'idle' && <StatusDot phase={dot} />}
        {n.id === 'logs' && problems > 0 && <span className="count-pill" title="Ошибок в журнале">{problems > 99 ? '99+' : problems}</span>}
      </button>
    );
  };

  return (
    <nav className="sidebar" aria-label="Навигация" ref={nav}>
      {pill && <span className="nav__pill" style={{ transform: `translateY(${pill.y}px)`, height: pill.h }} aria-hidden="true" />}
      {MAIN.map((n, i) => item(n, i))}
      <div className="nav__label">Системный прокси</div>
      {PROXY.map((n, i) => item(n, i + 4))}
      <div className="nav__label">Приложение</div>
      {APP.map((n, i) => item(n, i + 7))}
      <div className="sidebar__spacer" />
      <div className="mini-status glass">
        <div className="mini-status__row"><StatusDot phase={phase(status.tgws.state)} /><span className="mini-status__name">Telegram</span><b>{status.tgws.state === 'running' ? 'вкл' : 'выкл'}</b></div>
        <div className="mini-status__row"><StatusDot phase={phase(status.zapret.state)} /><span className="mini-status__name">Zapret</span><b>{status.zapret.state === 'running' ? 'вкл' : 'выкл'}</b></div>
        <div className="mini-status__row"><StatusDot phase={conn.phase} /><span className="mini-status__name">Прокси</span><b>{conn.phase === 'connected' ? 'вкл' : 'выкл'}</b></div>
      </div>
    </nav>
  );
}
