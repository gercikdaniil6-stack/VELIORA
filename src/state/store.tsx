import {
  createContext, useCallback, useContext, useEffect, useReducer, useRef, type ReactNode,
} from 'react';
import { native } from '../lib/native';
import type { Connection, Latency, PersistedState, Profile } from './types';

const KEY = 'veliora.state.v1';

export const DEFAULT_BYPASS = ['localhost', '127.*', '10.*', '192.168.*', '*.local'];

const DEFAULTS: PersistedState = {
  version: 1,
  profiles: [],
  rules: { bypass: DEFAULT_BYPASS, bypassLocal: true },
  automation: { connectOnLaunch: false, reapplyOnSwitch: true, autoProbe: true },
  prefs: { closeToTray: true, showSplash: true, reduceTransparency: false },
};

function load(): PersistedState {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const p = JSON.parse(raw) as Partial<PersistedState>;
    return {
      ...DEFAULTS,
      ...p,
      rules: { ...DEFAULTS.rules, ...p.rules },
      automation: { ...DEFAULTS.automation, ...p.automation },
      prefs: { ...DEFAULTS.prefs, ...p.prefs },
      profiles: Array.isArray(p.profiles) ? p.profiles : [],
    };
  } catch {
    return DEFAULTS;
  }
}

interface State {
  data: PersistedState;
  conn: Connection;
  latency: Latency;
}

type Action =
  | { t: 'data'; fn: (d: PersistedState) => PersistedState }
  | { t: 'conn'; conn: Connection }
  | { t: 'latency'; id: string; v: Latency[string] };

function reducer(s: State, a: Action): State {
  switch (a.t) {
    case 'data': return { ...s, data: a.fn(s.data) };
    case 'conn': return { ...s, conn: a.conn };
    case 'latency': return { ...s, latency: { ...s.latency, [a.id]: a.v } };
  }
}

export function bypassString(d: PersistedState) {
  const list = d.rules.bypass.map((x) => x.trim()).filter(Boolean);
  if (d.rules.bypassLocal) list.push('<local>');
  return list.join(';');
}

function useStoreValue() {
  const [s, dispatch] = useReducer(reducer, undefined, () => ({
    data: load(),
    conn: { phase: 'idle' } as Connection,
    latency: {} as Latency,
  }));
  const ref = useRef(s);
  ref.current = s;

  useEffect(() => {
    try { localStorage.setItem(KEY, JSON.stringify(s.data)); } catch { /* storage unavailable */ }
  }, [s.data]);

  const update = useCallback((fn: (d: PersistedState) => PersistedState) => dispatch({ t: 'data', fn }), []);

  const active = s.data.profiles.find((p) => p.id === s.data.activeId) ?? s.data.profiles[0];

  const connect = useCallback(async (profile?: Profile) => {
    const p = profile ?? ref.current.data.profiles.find((x) => x.id === ref.current.data.activeId) ?? ref.current.data.profiles[0];
    if (!p) return;
    dispatch({ t: 'conn', conn: { phase: 'connecting', profileId: p.id } });
    try {
      const server = await native.connect(p.kind, p.host, p.port, bypassString(ref.current.data));
      dispatch({ t: 'conn', conn: { phase: 'connected', since: Date.now(), server, profileId: p.id } });
      native.setTray(true, p.name);
    } catch (e) {
      dispatch({ t: 'conn', conn: { phase: 'error', error: String(e), profileId: p.id } });
      native.setTray(false);
    }
  }, []);

  const disconnect = useCallback(async () => {
    dispatch({ t: 'conn', conn: { ...ref.current.conn, phase: 'disconnecting' } });
    try {
      await native.disconnect();
      dispatch({ t: 'conn', conn: { phase: 'idle' } });
    } catch (e) {
      dispatch({ t: 'conn', conn: { phase: 'error', error: String(e) } });
    }
    native.setTray(false);
  }, []);

  const toggle = useCallback(() => {
    const ph = ref.current.conn.phase;
    if (ph === 'connected') return disconnect();
    if (ph === 'idle' || ph === 'error') return connect();
  }, [connect, disconnect]);

  const probe = useCallback(async (p: Profile) => {
    dispatch({ t: 'latency', id: p.id, v: { ...ref.current.latency[p.id], pending: true } });
    try {
      const ms = await native.latency(p.host, p.port);
      dispatch({ t: 'latency', id: p.id, v: { ms, at: Date.now() } });
    } catch (e) {
      dispatch({ t: 'latency', id: p.id, v: { error: String(e), at: Date.now() } });
    }
  }, []);

  const probeAll = useCallback(() => Promise.all(ref.current.data.profiles.map(probe)), [probe]);

  const setActive = useCallback(async (id: string) => {
    update((d) => ({ ...d, activeId: id }));
    const { conn, data } = ref.current;
    if (conn.phase === 'connected' && conn.profileId !== id && data.automation.reapplyOnSwitch) {
      const p = data.profiles.find((x) => x.id === id);
      if (p) await connect(p);
    }
  }, [update, connect]);

  return { ...s, active, update, connect, disconnect, toggle, probe, probeAll, setActive };
}

type Store = ReturnType<typeof useStoreValue>;
const Ctx = createContext<Store | null>(null);

export function StoreProvider({ children }: { children: ReactNode }) {
  const value = useStoreValue();
  const { data, toggle, connect, probeAll } = value;

  // Tray "Подключить/Отключить"
  useEffect(() => {
    let off = () => {};
    native.onTrayToggle(() => toggle()).then((f) => (off = f));
    return () => off();
  }, [toggle]);

  // Launch automation (once).
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    native.setTray(false);
    if (data.automation.autoProbe) probeAll();
    if (data.automation.connectOnLaunch && data.profiles.length) connect();
  }, [data, connect, probeAll]);

  // Periodic latency checks.
  useEffect(() => {
    if (!data.automation.autoProbe) return;
    const t = setInterval(probeAll, 60_000);
    return () => clearInterval(t);
  }, [data.automation.autoProbe, probeAll]);

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useStore() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useStore outside StoreProvider');
  return v;
}

export const uid = () => Math.random().toString(36).slice(2, 10) + Date.now().toString(36).slice(-4);
