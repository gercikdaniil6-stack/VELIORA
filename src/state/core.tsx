import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { api, DEFAULT_CONFIG, listen, type AppInfo, type Config, type CoreStatus, type LogEntry, type TestProgress, type TestReport } from '../lib/api';
import { useToast } from '../components/Toast';

export type Component = 'tgws' | 'zapret';
export const NAMES: Record<Component, string> = { tgws: 'Telegram-прокси', zapret: 'Zapret' };

const LOG_LIMIT = 3000;

function useCoreValue() {
  const toast = useToast();
  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG);
  const [loaded, setLoaded] = useState(false);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [status, setStatus] = useState<Record<Component, CoreStatus>>({ tgws: { state: 'stopped' }, zapret: { state: 'stopped' } });
  const [pending, setPending] = useState<Record<Component, boolean>>({ tgws: false, zapret: false });
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [tester, setTester] = useState<{ running: boolean; report: TestReport | null; progress: TestProgress | null }>({ running: false, report: null, progress: null });
  const cfgRef = useRef(config);
  cfgRef.current = config;

  const refreshInfo = useCallback(() => api.appInfo().then(setInfo).catch(() => {}), []);
  const refreshTester = useCallback(
    () => api.testerState().then((t) => setTester((s) => ({ ...s, running: t.running, report: t.report }))).catch(() => {}),
    [],
  );

  useEffect(() => {
    api.configGet().then((c) => { setConfig(c); setLoaded(true); }).catch(() => setLoaded(true));
    api.coreStatus().then((s) => setStatus(s)).catch(() => {});
    api.logsHistory().then(setLogs).catch(() => {});
    refreshInfo();
    refreshTester();
    const offs: Promise<() => void>[] = [
      listen<{ component: Component; status: CoreStatus }>('core://status', (p) => setStatus((s) => ({ ...s, [p.component]: p.status }))),
      listen<LogEntry>('log://entry', (e) => setLogs((l) => (l.length >= LOG_LIMIT ? [...l.slice(-LOG_LIMIT + 1), e] : [...l, e]))),
      listen<Config>('config://changed', setConfig),
      listen<TestProgress>('tester://progress', (p) => {
        const done = p.phase === 'done' || p.phase === 'cancelled' || p.phase === 'error';
        setTester((s) => ({ ...s, progress: p, running: !done }));
        if (done) {
          refreshTester();
          if (p.phase === 'done') toast.show({ tone: 'ok', title: 'Тестирование стратегий завершено', body: 'Лучшие варианты для вашей сети отмечены в списке стратегий.' });
          if (p.phase === 'cancelled') toast.show({ title: 'Тестирование остановлено', body: 'Уже проверенные стратегии сохранены в отчёте.' });
        }
      }),
    ];
    return () => { offs.forEach((o) => o.then((f) => f())); };
  }, [refreshInfo, refreshTester, toast]);

  useEffect(() => {
    document.documentElement.dataset.transparency = config.app.reduceTransparency ? 'reduced' : 'full';
  }, [config.app.reduceTransparency]);

  /** Applies a change; restarts a running component when its settings changed. */
  const save = useCallback(async (fn: (c: Config) => Config, opts: { quiet?: boolean } = {}) => {
    const next = fn(structuredClone(cfgRef.current));
    try {
      const r = await api.configSet(next);
      setConfig(r.config);
      for (const comp of r.restart) {
        const id = toast.show({ tone: 'busy', title: `Перезапускаем ${NAMES[comp]}…`, body: 'Чтобы новые настройки начали работать.' });
        try {
          await (comp === 'tgws' ? api.tgwsRestart() : api.zapretRestart());
          toast.update(id, { tone: 'ok', title: `${NAMES[comp]} перезапущен`, body: 'Настройки применены.' });
        } catch (e) {
          toast.update(id, { tone: 'error', title: `Настройки сохранены, но ${NAMES[comp]} не перезапустился`, body: String((e as Error).message) });
        }
      }
      if (!opts.quiet && r.restart.length === 0) toast.show({ tone: 'ok', title: 'Сохранено' });
      return true;
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не удалось сохранить', body: (e as Error).message });
      return false;
    }
  }, [toast]);

  const toggle = useCallback(async (comp: Component, on: boolean) => {
    setPending((p) => ({ ...p, [comp]: true }));
    try {
      const st = comp === 'tgws' ? await (on ? api.tgwsStart() : api.tgwsStop()) : await (on ? api.zapretStart() : api.zapretStop());
      setStatus((s) => ({ ...s, [comp]: st }));
    } catch (e) {
      toast.show({ tone: 'error', title: on ? `Не удалось запустить ${NAMES[comp]}` : `Не удалось остановить ${NAMES[comp]}`, body: (e as Error).message });
    } finally {
      setPending((p) => ({ ...p, [comp]: false }));
    }
  }, [toast]);

  const restart = useCallback(async (comp: Component) => {
    setPending((p) => ({ ...p, [comp]: true }));
    try {
      const st = await (comp === 'tgws' ? api.tgwsRestart() : api.zapretRestart());
      setStatus((s) => ({ ...s, [comp]: st }));
      return true;
    } catch (e) {
      toast.show({ tone: 'error', title: `Не удалось перезапустить ${NAMES[comp]}`, body: (e as Error).message });
      return false;
    } finally {
      setPending((p) => ({ ...p, [comp]: false }));
    }
  }, [toast]);

  return { config, loaded, info, status, pending, logs, setLogs, tester, setTester, save, toggle, restart, refreshInfo, refreshTester };
}

type Core = ReturnType<typeof useCoreValue>;
const Ctx = createContext<Core | null>(null);

export function CoreProvider({ children }: { children: ReactNode }) {
  const value = useCoreValue();
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useCore() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useCore outside CoreProvider');
  return v;
}

export const busy = (s: CoreStatus) => s.state === 'starting' || s.state === 'stopping';
