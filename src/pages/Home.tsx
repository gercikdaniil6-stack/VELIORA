import { lazy, Suspense, useState } from 'react';
import { BrandLogo } from '../brand/BrandLogo';
import type { Route } from '../components/Sidebar';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Banner, Spinner, useUptime } from '../components/ui';
import { api, type CoreStatus } from '../lib/api';
import { busy, useCore, type Component } from '../state/core';
const Diagnostics = lazy(() => import('../components/Diagnostics').then(m => ({ default: m.Diagnostics })));

const WORD: Record<CoreStatus['state'], string> = {
  stopped: 'Отключено',
  starting: 'Подключение',
  running: 'Подключено',
  stopping: 'Отключение',
  error: 'Ошибка',
};

function PowerOrb({ label, comp, status, pending, version, onToggle, disabled, note }: {
  label: string; comp: Component; status: CoreStatus; pending: boolean; version?: string;
  onToggle: () => void; disabled?: boolean; note?: string;
}) {
  const uptime = useUptime(status);
  const working = pending || busy(status);
  const on = status.state === 'running';
  return (
    <div className={`orb orb--${status.state}${working ? ' orb--busy' : ''}`} data-comp={comp}>
      <div className="orb__label">{label}</div>
      <div className="orb__state" aria-live="polite">{comp === 'tgws' && working ? (status.state === 'stopping' ? 'Выключение' : 'Подбор сервера') : comp === 'tgws' && on ? 'Прокси включён' : WORD[working && status.state === 'stopped' ? 'starting' : status.state]}</div>
      <button className="orb__button" onClick={onToggle} disabled={working || disabled} aria-pressed={on}
        aria-label={`${label}: ${on ? 'выключить' : 'включить'}`} title={disabled ? note : undefined}>
        <span className="orb__halo" aria-hidden="true" />
        <span className="orb__glass" aria-hidden="true" />
        <span className="orb__lens" aria-hidden="true" />
        <span className="orb__icon" aria-hidden="true">{working ? <Spinner size={34} /> : <Icon name={on ? 'pause' : 'power'} size={40} stroke={2} />}</span>
      </button>
      <div className={`orb__timer mono${on ? ' orb__timer--on' : ''}`}>{uptime}</div>
      {version && <div className="orb__ver">v{version}</div>}
      {note && disabled && <div className="orb__note">{note}</div>}
      {status.state === 'error' && status.lastError && <div className="orb__error" title={status.lastError}>{status.lastError}</div>}
    </div>
  );
}

export function Home({ onNavigate, onAddSite }: { onNavigate: (r: Route) => void; onAddSite: () => void }) {
  const core = useCore();
  const toast = useToast();
  const { status, pending, info, config, tester } = core;
  const [diag, setDiag] = useState(false);
  const [hiddenVersion, setHiddenVersion] = useState<string>();
  const [reloading, setReloading] = useState(false);

  const latest = core.versions.releases.find((release) => release.latest);
  const update = latest && (latest.tag !== info?.zapretInstalled);

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
    const restartRelay = core.relay.state === 'running';
    if (!targets.length && !restartRelay) {
      toast.show({ title: 'Нечего перезапускать', body: 'Включите Telegram-прокси или Zapret.' });
      return;
    }
    setReloading(true);
    const jobs = targets.map((c) => core.restart(c));
    if (restartRelay) jobs.push(core.power(true));
    const ok = await Promise.all(jobs);
    setReloading(false);
    if (ok.every(Boolean)) toast.show({ tone: 'ok', title: 'Перезапуск завершён' });
  };

  const zapretNote = tester.running ? 'Идёт тест стратегий' : info && !info.admin ? 'Нужны права администратора' : undefined;

  return (
    <div className="page page--home">
      {info && !info.admin && (
        <Banner tone="warn" icon="shield" title="VELIORA App работает без прав администратора"
          actions={<button className="btn btn--primary" onClick={() => api.elevate().catch((e) => toast.show({ tone: 'error', title: 'Права не получены', body: e.message }))}>Запросить права</button>}>
          Zapret и тест стратегий недоступны: драйверу WinDivert нужны права администратора. Telegram-прокси работает и так.
        </Banner>
      )}
      <header className="page-head">
        <div><h1>Главная</h1><p>Стратегия Zapret · {config.zapret.strategy?.replace(/\.bat$/i, '') ?? 'не выбрана'}</p></div>
        <button className="btn" onClick={() => { void core.refreshVersions(); onNavigate('zapret'); }}><Icon name="refresh" /> Проверить версии Zapret</button>
      </header>
      {info && update && latest.tag !== hiddenVersion && (
        <Banner tone="accent" title={`Доступна сборка Zapret ${latest.tag}`}
          actions={<><button className="btn" onClick={() => onNavigate('zapret')}>К версиям</button><button className="btn btn--ghost btn--icon" aria-label="Скрыть" onClick={() => setHiddenVersion(latest.tag)}><Icon name="close" size={14} /></button></>}>
          Установлена {info.zapretInstalled}.
        </Banner>
      )}

      <section className="home-stage">
        <PowerOrb label="Telegram" comp="tgws" status={core.relay} pending={core.relayPending} onToggle={() => core.power(core.relay.state !== 'running')} />
        <div className="home-stage__mark" aria-hidden="true">
          <BrandLogo variant="symbol" size={56} decorative />
        </div>
        <PowerOrb label="Zapret" comp="zapret" status={status.zapret} pending={pending.zapret} version={info?.zapretInstalled ?? '…'}
          onToggle={() => toggle('zapret')} disabled={!!zapretNote} note={zapretNote} />
      </section>

      <div className="home-actions">
        <button className="btn" onClick={reloadAll} disabled={reloading || busy(core.relay) || core.relayPending}>
          {reloading ? <Spinner size={14} /> : <Icon name="refresh" size={16} />} Перезапустить процессы
        </button>
        <button className="btn" onClick={() => setDiag(true)}>
          <Icon name="stethoscope" size={16} /> Почему может не работать?
        </button>
      </div>

      <div className="grid grid--3 home-tiles">
        <button className="tile glass" onClick={() => onNavigate('catalog')}>
          <span className="tile__icon"><Icon name="layers" /></span>
          <span className="tile__text"><span className="tile__title">Открыть каталог</span><span className="tile__sub">Готовые правила для сервисов</span></span>
          <Icon name="chevron" size={16} />
        </button>
        <button className="tile glass" onClick={onAddSite}>
          <span className="tile__icon"><Icon name="plus" /></span>
          <span className="tile__text"><span className="tile__title">Добавить сайт</span><span className="tile__sub">Домены и ссылки</span></span>
          <Icon name="chevron" size={16} />
        </button>
        <button className="tile glass" onClick={() => onNavigate('zapret')}>
          <span className="tile__icon"><Icon name="shield" /></span>
          <span className="tile__text"><span className="tile__title">Выбрать стратегию</span><span className="tile__sub">Выбор и тестирование</span></span>
          <Icon name="chevron" size={16} />
        </button>
      </div>
      {diag && <Suspense fallback={null}><Diagnostics open onClose={() => setDiag(false)} /></Suspense>}
    </div>
  );
}
