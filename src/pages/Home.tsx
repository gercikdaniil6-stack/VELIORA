import { useEffect, useState } from 'react';
import { BrandLogo } from '../brand/BrandLogo';
import type { Route } from '../components/Sidebar';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Banner, Spinner, useUptime } from '../components/ui';
import { Diagnostics } from '../components/Diagnostics';
import { api, type CoreStatus, type UpdateCheck } from '../lib/api';
import { busy, useCore, type Component } from '../state/core';
import { useStore } from '../state/store';
import { phaseTitle } from '../components/bits';

const WORD: Record<CoreStatus['state'], string> = {
  stopped: 'Отключено',
  starting: 'Подключение',
  running: 'Подключено',
  stopping: 'Отключение',
  error: 'Ошибка',
};

function PowerOrb({ label, comp, status, pending, version, onToggle, disabled, note }: {
  label: string; comp: Component; status: CoreStatus; pending: boolean; version: string;
  onToggle: () => void; disabled?: boolean; note?: string;
}) {
  const uptime = useUptime(status);
  const working = pending || busy(status);
  const on = status.state === 'running';
  return (
    <div className={`orb orb--${status.state}${working ? ' orb--busy' : ''}`} data-comp={comp}>
      <div className="orb__label">{label}</div>
      <div className="orb__state" aria-live="polite">{WORD[working && status.state === 'stopped' ? 'starting' : status.state]}</div>
      <button className="orb__button" onClick={onToggle} disabled={working || disabled} aria-pressed={on}
        aria-label={`${label}: ${on ? 'выключить' : 'включить'}`} title={disabled ? note : undefined}>
        <span className="orb__halo" aria-hidden="true" />
        <span className="orb__glass" aria-hidden="true" />
        <span className="orb__lens" aria-hidden="true" />
        <span className="orb__icon" aria-hidden="true">{working ? <Spinner size={34} /> : <Icon name={on ? 'pause' : 'power'} size={40} stroke={2} />}</span>
      </button>
      <div className={`orb__timer mono${on ? ' orb__timer--on' : ''}`}>{uptime}</div>
      <div className="orb__ver">v{version}</div>
      {note && disabled && <div className="orb__note">{note}</div>}
      {status.state === 'error' && status.lastError && <div className="orb__error" title={status.lastError}>{status.lastError}</div>}
    </div>
  );
}

export function Home({ onNavigate }: { onNavigate: (r: Route) => void }) {
  const core = useCore();
  const toast = useToast();
  const proxy = useStore();
  const { status, pending, info, config, tester } = core;
  const [diag, setDiag] = useState(false);
  const [updates, setUpdates] = useState<UpdateCheck | null>(null);
  const [hidden, setHidden] = useState<Record<string, boolean>>({});
  const [reloading, setReloading] = useState(false);

  useEffect(() => {
    if (!config.app.autoCheckUpdates) return;
    api.checkUpdates().then(setUpdates).catch(() => {});
  }, [config.app.autoCheckUpdates]);

  const toggle = (comp: Component) => {
    const on = status[comp].state !== 'running';
    if (comp === 'zapret' && on && !config.zapret.strategy) {
      toast.show({ title: 'Сначала выберите стратегию', body: 'Запустите тест — он подберёт стратегию для вашей сети.' });
      onNavigate('zapret');
      return;
    }
    core.toggle(comp, on);
  };

  const reloadAll = async () => {
    const targets = (['tgws', 'zapret'] as Component[]).filter((c) => status[c].state === 'running' || status[c].state === 'error');
    if (!targets.length) {
      toast.show({ title: 'Нечего перезапускать', body: 'Включите Telegram-прокси или Zapret.' });
      return;
    }
    setReloading(true);
    const ok = await Promise.all(targets.map((c) => core.restart(c)));
    setReloading(false);
    if (ok.every(Boolean)) toast.show({ tone: 'ok', title: 'Процессы перезапущены' });
  };

  const zapretNote = tester.running ? 'Идёт тест стратегий' : info && !info.admin ? 'Нужны права администратора' : undefined;
  const conn = proxy.conn;

  return (
    <div className="page page--home">
      {info && !info.admin && (
        <Banner tone="warn" icon="shield" title="VELIORA App работает без прав администратора"
          actions={<button className="btn btn--primary" onClick={() => api.elevate().catch((e) => toast.show({ tone: 'error', title: 'Права не получены', body: e.message }))}>Запросить права</button>}>
          Zapret и тест стратегий недоступны: драйверу WinDivert нужны права администратора. Telegram-прокси работает и так.
        </Banner>
      )}
      {updates?.zapretUpdate && !hidden.zapret && (
        <Banner tone="accent" title={`Доступна новая сборка Zapret — ${updates.zapretLatest}`}
          actions={<><button className="btn" onClick={() => onNavigate('zapret')}>Подробнее</button><button className="btn btn--ghost btn--icon" aria-label="Скрыть" onClick={() => setHidden((h) => ({ ...h, zapret: true }))}><Icon name="close" size={14} /></button></>}>
          Установлена {updates.zapretInstalled}. Списки, которые вы добавили, перенесутся на новую версию.
        </Banner>
      )}

      <section className="home-stage">
        <PowerOrb label="Telegram" comp="tgws" status={status.tgws} pending={pending.tgws} version={info?.tgwsVersion ?? '…'} onToggle={() => toggle('tgws')} />
        <div className="home-stage__mark" aria-hidden="true">
          <BrandLogo variant="symbol" size={56} decorative />
        </div>
        <PowerOrb label="Zapret" comp="zapret" status={status.zapret} pending={pending.zapret} version={info?.zapretInstalled ?? '…'}
          onToggle={() => toggle('zapret')} disabled={!!zapretNote} note={zapretNote} />
      </section>

      <div className="home-actions">
        <button className="btn" onClick={reloadAll} disabled={reloading}>
          {reloading ? <Spinner size={14} /> : <Icon name="refresh" size={16} />} Перезапустить процессы
        </button>
        <button className="btn" onClick={() => setDiag(true)}>
          <Icon name="stethoscope" size={16} /> Почему может не работать?
        </button>
      </div>

      <div className="grid grid--3 home-tiles">
        <button className="tile glass" onClick={() => onNavigate('telegram')}>
          <span className="tile__icon"><Icon name="telegram" /></span>
          <span className="tile__text">
            <span className="tile__title">Telegram-прокси</span>
            <span className="tile__sub mono">{config.tgws.host}:{config.tgws.port}</span>
          </span>
          <Icon name="chevron" size={16} />
        </button>
        <button className="tile glass" onClick={() => onNavigate('zapret')}>
          <span className="tile__icon"><Icon name="shield" /></span>
          <span className="tile__text">
            <span className="tile__title">Стратегия Zapret</span>
            <span className="tile__sub">{config.zapret.strategy ? config.zapret.strategy.replace(/\.bat$/i, '') : 'не выбрана'}</span>
          </span>
          <Icon name="chevron" size={16} />
        </button>
        <button className="tile glass" onClick={() => onNavigate('servers')}>
          <span className="tile__icon"><Icon name="globe" /></span>
          <span className="tile__text">
            <span className="tile__title">Системный прокси</span>
            <span className="tile__sub">{phaseTitle(conn.phase)}{conn.server ? ` · ${conn.server}` : ''}</span>
          </span>
          <Icon name="chevron" size={16} />
        </button>
      </div>

      <Diagnostics open={diag} onClose={() => setDiag(false)} />
    </div>
  );
}
