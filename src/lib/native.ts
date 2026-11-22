// Bridge to the Rust side. When the UI is opened in a plain browser (vite dev
// preview) there is no Tauri runtime: calls are simulated and the title bar
// shows a "browser preview" badge, so the preview is never mistaken for the app.
import type { ProxyKind } from '../state/types';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export interface ProxyStatus {
  enabled: boolean;
  server?: string | null;
  bypass?: string | null;
}

export const native = {
  async connect(kind: ProxyKind, host: string, port: number, bypass: string): Promise<string> {
    if (!isTauri) { await wait(650); return `${host}:${port}`; }
    const r = await invoke<{ server: string }>('proxy_connect', { kind, host, port, bypass });
    return r.server;
  },
  async disconnect(): Promise<void> {
    if (!isTauri) { await wait(300); return; }
    await invoke('proxy_disconnect');
  },
  async status(): Promise<ProxyStatus> {
    if (!isTauri) return { enabled: false };
    return invoke<ProxyStatus>('proxy_status');
  },
  async latency(host: string, port: number): Promise<number> {
    if (!isTauri) {
      await wait(300 + Math.random() * 400);
      // deterministic per host so the preview is stable
      let h = 0; for (const c of host) h = (h * 31 + c.charCodeAt(0)) % 997;
      return 18 + (h % 140);
    }
    return invoke<number>('probe_latency', { host, port });
  },
  async setTray(connected: boolean, label?: string) {
    if (isTauri) await invoke('set_tray_state', { connected, label: label ?? null });
  },
  async quit() {
    if (isTauri) await invoke('quit_app');
  },
  async version(): Promise<string> {
    if (!isTauri) return '1.0.0';
    const { getVersion } = await import('@tauri-apps/api/app');
    return getVersion();
  },
  async onTrayToggle(cb: () => void): Promise<() => void> {
    if (!isTauri) return () => {};
    const { listen } = await import('@tauri-apps/api/event');
    return listen('tray://toggle', cb);
  },
  window: {
    async minimize() { if (isTauri) (await win()).minimize(); },
    async toggleMaximize() { if (isTauri) (await win()).toggleMaximize(); },
    async close() { if (isTauri) (await win()).close(); },
    async isMaximized() { return isTauri ? (await win()).isMaximized() : false; },
    async onResized(cb: () => void): Promise<() => void> {
      if (!isTauri) return () => {};
      return (await win()).onResized(cb);
    },
  },
};

async function win() {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}
