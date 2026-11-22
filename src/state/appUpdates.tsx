
import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { Modal } from '../components/ui';
import { Setting } from '../components/bits';
import { useCore } from './core';
import { api } from '../lib/api';

const CHECK_INTERVAL = 6 * 60 * 60 * 1000;
const SNOOZE = 4 * 60 * 60 * 1000;
type Offer = { version: string; notes: string };
type CheckResult = { latest: string | null; offer: Offer | null };
type Progress = { downloaded: number; total: number | null; installing: boolean };
type UpdateState = {
  offer: Offer | null; checkedAt: number | null; checking: boolean;
  error: string; busy: boolean; latest: string | null;
  check: () => Promise<void>; show: () => void;
};
const Context = createContext<UpdateState | null>(null);
function formatDate(value: number | null) {
  return value ? new Date(value).toLocaleString('ru-RU') : 'Ещё не проверялось';
}
function message(error: unknown) {
  return typeof error === 'string' ? error : error instanceof Error ? error.message : 'Не удалось обновить приложение.';
}

export function AppUpdateProvider({ children }: { children: ReactNode }) {
  const { loaded, config } = useCore();
  const [offer, setOffer] = useState<Offer | null>(null);
  const [latest, setLatest] = useState<string | null>(null);
  const [checkedAt, setCheckedAt] = useState<number | null>(null);
  const [checking, setChecking] = useState(false);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState('');
  const active = useRef(false);
  const snoozeTimer = useRef<number | null>(null);

  const performCheck = useCallback(async (manual: boolean) => {
    if (!isTauri() || active.current) return;
    active.current = true;
    setChecking(true);
    setError('');
    try {
      const { offer: update, latest: version } = await invoke<CheckResult>('app_update_check');
      setLatest(version);
      setOffer(update);
      setCheckedAt(Date.now());
      let reminder = 0;
      try { if (update) reminder = Number(localStorage.getItem('veliora.update.snooze.' + update.version)); } catch { /* optional preference */ }
      setOpen(Boolean(update && (manual || reminder <= Date.now())));
    } catch (cause) {
      setError(message(cause));
    } finally {
      setChecking(false);
      active.current = false;
    }
  }, []);

  useEffect(() => {
    if (!loaded || !config.app.appAutoCheckUpdates) return;
    const startup = window.setTimeout(() => { void performCheck(false); }, 1800);
    const interval = window.setInterval(() => { void performCheck(false); }, CHECK_INTERVAL);
    return () => { window.clearTimeout(startup); window.clearInterval(interval); };
  }, [loaded, config.app.appAutoCheckUpdates, performCheck]);
  useEffect(() => () => { if (snoozeTimer.current !== null) window.clearTimeout(snoozeTimer.current); }, []);

  const remind = () => {
    if (active.current) return;
    setOpen(false);
    if (offer) {
      try { localStorage.setItem('veliora.update.snooze.' + offer.version, String(Date.now() + SNOOZE)); } catch { /* optional preference */ }
      if (snoozeTimer.current !== null) window.clearTimeout(snoozeTimer.current);
      if (config.app.appAutoCheckUpdates) {
        snoozeTimer.current = window.setTimeout(() => { void performCheck(false); }, SNOOZE);
      }
    }
  };
  const install = async () => {
    if (!offer || active.current) return;
    active.current = true;
    setBusy(true);
    setError('');
    setProgress(null);
    const channel = new Channel<Progress>((event) => setProgress(event));
    try {
      await invoke('app_update_install', { version: offer.version, progress: channel });
    } catch (cause) {
      setError(message(cause));
    } finally {
      active.current = false;
      setBusy(false);
    }
  };
  const percent = progress?.total ? Math.min(100, Math.floor(progress.downloaded * 100 / progress.total)) : undefined;
  return (
    <Context.Provider value={{ offer, checkedAt, checking, error, busy, latest, check: () => performCheck(true), show: () => setOpen(true) }}>
      {children}
      <Modal open={open && offer !== null} onClose={remind} title={'VELIORA ' + (offer?.version ?? '')}
        lead="Доступна новая версия приложения"
        foot={<>
          <button className="btn btn--ghost" disabled={busy} onClick={remind}>Напомнить позже</button>
          <button className="btn btn--primary" disabled={busy} onClick={() => void install()}>Обновить сейчас</button>
        </>}>
        <h3>Что изменилось</h3>
        <div className="app-update-notes">{offer?.notes || 'Описание изменений отсутствует.'}</div>
        {busy && <div className="app-update-progress" aria-live="polite">
          <div>{progress?.installing ? 'Запускается установщик Windows…' : percent === undefined ? 'Загрузка обновления…' : 'Загрузка · ' + percent + '%'}</div>
          <progress aria-label="Загрузка обновления" max={100} value={percent} />
        </div>}
        {error && <p role="alert">{error}</p>}
      </Modal>
    </Context.Provider>
  );
}

export function AppUpdatesSettings() {
  const core = useCore();
  const state = useContext(Context);
  if (!state) return null;
  return <section className="glass card">
    <h2>Обновления VELIORA</h2>
    <Setting title="Проверять обновления автоматически" desc="При запуске и каждые 6 часов." checked={core.config.app.appAutoCheckUpdates}
      onChange={(appAutoCheckUpdates) => { void core.save(c => ({ ...c, app: { ...c.app, appAutoCheckUpdates } })); }} />
    <dl className="app-update-details">
      <dt>Текущая версия</dt><dd>{core.info?.version ?? '—'}</dd>
      <dt>Доступная версия</dt><dd>{state.latest ?? (state.checkedAt && !state.error ? 'Обновлений нет' : '—')}</dd>
      <dt>Последняя успешная проверка</dt><dd>{formatDate(state.checkedAt)}</dd>
    </dl>
    <div className="app-update-actions">
      <button className="btn" onClick={() => void state.check()} disabled={state.checking || state.busy || !isTauri()}>
        {state.checking ? 'Проверяем…' : 'Проверить обновления'}
      </button>
      {state.offer && <button className="btn btn--primary" onClick={state.show}>Посмотреть обновление</button>}
    </div>
    {state.error && <p role="alert">{state.error}</p>}
    <details>
      <summary>История изменений</summary>
      {state.offer?.notes && <div className="app-update-notes">{state.offer.notes}</div>}
      <button className="btn btn--ghost" onClick={() => void api.openUrl('https://github.com/gercikdaniil6-stack/VELIORA/releases')}>Все выпуски на GitHub</button>
    </details>
  </section>;
}

