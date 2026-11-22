import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { api, DEFAULT_CONFIG, listen, type AppInfo, type Config, type CoreStatus, type ZapretConflict, type Release, type ManagedSite, type TestProgress, type TestReport } from '../lib/api';
import { useToast } from '../components/Toast';

export type Component = 'tgws' | 'zapret';
export const NAMES: Record<Component, string> = { tgws: 'Локальный прокси', zapret: 'Zapret' };


function useCoreValue() {
  const toast = useToast();
  const [relay, setRelay] = useState<CoreStatus>({ state: 'stopped' });
  const [relayPending, setRelayPending] = useState(false);
  useEffect(() => {
    let mounted = true;
    const off = listen<CoreStatus>('telegram://power', (s) => { if (mounted) setRelay(s); });
    off.then(() => api.telegramPowerStatus()).then(s => { if (mounted) setRelay(s); }).catch(() => {});
    return () => { mounted = false; off.then(unlisten => unlisten()); };
  }, []);
  const power = useCallback(async (on: boolean, id = 0) => {
    setRelayPending(true);
    try {
      await api.telegramPower(on, id);
      return true;
    } catch (e) {
      toast.show({ tone: 'error', title: on ? 'Не удалось включить прокси' : 'Не удалось выключить прокси', body: (e as Error).message });
      return false;
    } finally {
      setRelayPending(false);
    }
  }, [toast]);
  const [config, setConfig] = useState<Config>(DEFAULT_CONFIG);
  const [loaded, setLoaded] = useState(false);
  const [versions, setVersions] = useState<{ releases: Release[]; error: string | null; loading: boolean }>({ releases: [], error: null, loading: false });
  const versionRequest = useRef<Promise<void> | null>(null);
  const refreshVersions = useCallback(() => {
    if (versionRequest.current) return versionRequest.current;
    setVersions((v) => ({ ...v, loading: true }));
    const request = api.releases().then((result) => {
      setVersions((previous) => ({
        releases: result.error ? [
          ...result.releases,
          ...previous.releases.filter((old) => !result.releases.some((r) => r.tag === old.tag)),
        ] : result.releases,
        error: result.error, loading: false,
      }));
    }).catch((e: Error) => setVersions((previous) => ({ ...previous, error: e.message, loading: false })))
      .finally(() => { versionRequest.current = null; });
    versionRequest.current = request;
    return request;
  }, []);
  useEffect(() => {
    if (loaded && config.app.autoCheckUpdates) void refreshVersions();
  }, [loaded, config.app.autoCheckUpdates, refreshVersions]);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [status, setStatus] = useState<Record<Component, CoreStatus>>({ tgws: { state: 'stopped' }, zapret: { state: 'stopped' } });
  const [pending, setPending] = useState<Record<Component, boolean>>({ tgws: false, zapret: false });
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
    refreshInfo();
    refreshTester();
    const offs: Promise<() => void>[] = [
      listen<{ component: Component; status: CoreStatus }>('core://status', (p) => setStatus((s) => ({ ...s, [p.component]: p.status }))),
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

  const [conflict, setConflict] = useState<ZapretConflict>();
  const inspectConflict = useCallback(async () => {
    const result = await api.zapretConflicts().catch(() => null);
    if (!result?.conflicts.length) return false;
    setConflict(result.conflicts.find((item) => item.token) ?? result.conflicts[0]);
    return true;
  }, []);

  useEffect(() => {
    const notify = listen<string>('zapret://maintenance', (title) => toast.show({ title }));
    const inspect = listen('zapret://conflict-offer', () => { void inspectConflict(); });
    return () => { void notify.then((remove) => remove()); void inspect.then((remove) => remove()); };
  }, [inspectConflict, toast]);

  const resolveConflict = useCallback(async (token: string, remember: boolean) => {
    setPending((p) => ({ ...p, zapret: true }));
    try {
      const result = await api.resolveZapretConflict(token, remember);
      setStatus((s) => ({ ...s, zapret: result }));
      setConflict(undefined);
    } catch (e) {
      if (!await inspectConflict()) setConflict(undefined);
      toast.show({ tone: 'error', title: 'Не удалось включить Zapret', body: (e as Error).message });
    } finally {
      setPending((p) => ({ ...p, zapret: false }));
    }
  }, [inspectConflict, toast]);

  const toggle = useCallback(async (comp: Component, on: boolean) => {
    setPending((p) => ({ ...p, [comp]: true }));
    try {
      const st = comp === 'tgws' ? await (on ? api.tgwsStart() : api.tgwsStop()) : await (on ? api.zapretStart() : api.zapretStop());
      setStatus((s) => ({ ...s, [comp]: st }));
    } catch (e) {
      if (comp === 'zapret' && on && await inspectConflict()) return;
      toast.show({ tone: 'error', title: on ? `Не удалось запустить ${NAMES[comp]}` : `Не удалось остановить ${NAMES[comp]}`, body: (e as Error).message });
    } finally {
      setPending((p) => ({ ...p, [comp]: false }));
    }
  }, [inspectConflict, toast]);

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

  const saveManaged = useCallback(async (sites: ManagedSite[], services: string[]) => {
    try {
      const { revision, restarted } = await api.saveSites(cfgRef.current.zapret.managedRevision ?? 0, sites, services);
      setConfig((current) => (current.zapret.managedRevision ?? 0) >= revision ? current : ({
        ...current, zapret: { ...current.zapret, managedSites: sites, managedServices: services, managedRevision: revision },
      }));
      toast.show({ tone: 'ok', title: 'Список сохранён', body: restarted ? 'Zapret перезапущен с новым списком.' : undefined, ms: 2300 });
      return true;
    } catch (e) {
      toast.show({ tone: 'error', title: 'Список не сохранён', body: (e as Error).message });
      return false;
    }
  }, [toast]);

  return { config, loaded, info, conflict, setConflict, resolveConflict, versions, refreshVersions, saveManaged, relay, power, relayPending, status, pending, tester, setTester, save, toggle, restart, refreshInfo, refreshTester };
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
