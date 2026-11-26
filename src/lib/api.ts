// Typed bridge to the Rust core (Telegram proxy, Zapret, tester, logs…).
// In a plain browser (vite preview) there is no Tauri runtime: calls return
// neutral defaults so the UI still renders; actions report "only in the app".
import type { Channel } from '@tauri-apps/api/core';
import { isTauri } from './native';

export type CoreState = 'stopped' | 'starting' | 'running' | 'stopping' | 'error';

export interface CoreStatus {
  state: CoreState;
  pid?: number | null;
  startedAt?: number | null;
  lastError?: string | null;
  detail?: string | null;
}

export interface TgwsConfig { host: string; port: number; secret: string; workerDomain: string; autoStart: boolean; verbose: boolean }
export type DiscordUdp = 'bundle' | 'discovery' | 'direct';
export interface DiscordVerified { strategy: string; udp: DiscordUdp; version: string; at: number }
export interface DiscordDiagnostics {
  web: Probe[];
  udp: { sent: number; received: number; averageMs: number | null; error: string | null };
  status: CoreStatus;
  captureConfirmed: boolean;
  competing: number;
  mtu: number[];
  vpn: boolean;
}
export interface ManagedSite { name?: string | null; domain: string; enabled: boolean; strategy: string | null }
export interface ZapretConflict {
  label: string;
  image: string;
  path?: string | null;
  pid: number;
  confirmed: boolean;
  signatureVerified?: boolean | null;
  token?: string | null;
}

export interface ZapretConfig {
  strategy: string | null;
  autoStart: boolean;
  gameFilter: 'off' | 'all' | 'tcp' | 'udp';
  gameTcp: string;
  gameUdp: string;
  ipsetMode: 'basic' | 'full' | 'any';
  fakeDiscord: string | null;
  fakeGame: string | null;
  autoResolveConflicts?: boolean;
  discordUdp?: DiscordUdp;
  discordVerified?: DiscordVerified | null;
  managedSites?: ManagedSite[];
  managedServices?: string[];
  managedRevision?: number;
}
export interface AppPrefs {
  launchAtLogin: boolean;
  silentStart: boolean;
  tray: boolean;
  closeToTray: boolean;
  hideTaskbar: boolean;
  autoCheckUpdates: boolean;
  appAutoCheckUpdates: boolean;
  showSplash: boolean;
  reduceTransparency: boolean;
}
export interface Config { tgws: TgwsConfig; zapret: ZapretConfig; app: AppPrefs; appearance?: unknown }

export interface AppInfo {
  version: string;
  admin: boolean;
  elevationDeclined: boolean;
  elevationError: number;
  autostarted: boolean;
  dataDir: string;
  zapretBundled: string;
  zapretInstalled: string;
  zapretSource: string;
  zapretDir: string;
  tgwsVersion: string;
  autorunRegistered: boolean;
}

export interface Strategy { file: string; name: string; note?: string | null; gameFilter: boolean; fakeDiscord: boolean; fakeGame: boolean }
export interface FakeFile { file: string; label: string }
export interface ZapretOverview { strategies: Strategy[]; fakes: FakeFile[]; ipsetFull: number; version: string; source: string; ready: boolean; admin: boolean }

export interface ListInfo { file: string; title: string; desc: string; kind: 'domains' | 'ips'; user: boolean; count: number; hasBackup: boolean }
export interface WriteResult { saved: number; rejected: string[] }

export interface LogEntry { id: number; time: number; source: 'app' | 'tgws' | 'zapret' | 'proxy' | 'test'; level: 'info' | 'warn' | 'error'; text: string }

export interface Probe { service: string; url: string; ok: boolean; ms: number; fault: string }
export interface StrategyResult {
  status: 'passed' | 'failed' | 'not_started';
  opened: number;
  blockedTotal: number;
  broken: number;
  latencyMs?: number | null;
  services: Record<string, [number, number]>;
  score: number;
  rechecked: boolean;
  startupError?: string | null;
}
export interface TestReport {
  ranAt: number;
  durationMs: number;
  bundleVersion: string;
  cancelled: boolean;
  vpn?: string | null;
  systemProxy: boolean;
  baseline: Probe[];
  blocked: string[];
  unfixable: string[];
  dnsIssues: string[];
  results: Record<string, StrategyResult>;
  best: string[];
  tested: number;
  total: number;
}
export interface TestProgress { phase: 'baseline' | 'testing' | 'recheck' | 'done' | 'cancelled' | 'error'; current: number; total: number; strategy?: string | null; message?: string | null }

export interface Release { tag: string; name: string; publishedAt: string; notes: string; assetName?: string | null; assetUrl?: string | null; size: number; sha256?: string | null; installed: boolean; latest: boolean }
export interface UpdateCheck {
  zapretInstalled: string; zapretLatest?: string | null; zapretUpdate: boolean;
  tgwsInstalled: string; tgwsLatest?: string | null; tgwsUpdate: boolean;
  errors: string[];
}

export interface DiagAction { kind: string; label: string }
export interface DiagIssue { id: string; severity: 'error' | 'warn'; title: string; impact: string; detail: string; steps: string[]; commands: string[]; actions: DiagAction[] }
export interface DiagReport { issues: DiagIssue[]; passed: string[]; admin: boolean; closeOffer?: { token: string; label: string } | null; checks: { name: string; status: string; detail: string }[] }

export const PREVIEW_ERROR = 'Доступно только в приложении VELIORA App';

async function call<T>(cmd: string, args?: Record<string, unknown>, preview?: T): Promise<T> {
  if (!isTauri) {
    if (preview !== undefined) return preview;
    throw new Error(PREVIEW_ERROR);
  }
  const { invoke } = await import('@tauri-apps/api/core');
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e));
  }
}

export async function listen<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  return listen<T>(event, (e) => cb(e.payload));
}

const STOPPED: CoreStatus = { state: 'stopped' };
export const DEFAULT_CONFIG: Config = {
  tgws: { host: '127.0.0.1', port: 1443, secret: '0123456789abcdef0123456789abcdef', workerDomain: '', autoStart: false, verbose: false },
  zapret: { strategy: null, autoStart: false, gameFilter: 'off', gameTcp: '1024-65535', gameUdp: '1024-65535', ipsetMode: 'basic', fakeDiscord: null, fakeGame: null },
  app: { launchAtLogin: false, silentStart: false, tray: true, closeToTray: true, hideTaskbar: false, autoCheckUpdates: true, appAutoCheckUpdates: true, showSplash: true, reduceTransparency: false },
};

export const api = {
  appInfo: () => call<AppInfo>('app_info', undefined, {
    version: '2.0.0', admin: false, elevationDeclined: false, elevationError: 0, autostarted: false, dataDir: '', zapretBundled: '1.10.4',
    zapretInstalled: '1.10.4', zapretSource: 'bundled', zapretDir: '', tgwsVersion: '1.11.4', autorunRegistered: false,
  }),
  elevate: () => call<void>('app_elevate'),
  restart: () => call<void>('app_restart'),
  openUrl: (url: string) => call<void>('open_url', { url }),

  configGet: () => call<Config>('config_get', undefined, DEFAULT_CONFIG),
  configSet: (config: Config) => call<{ config: Config; restart: ('tgws' | 'zapret')[] }>('config_set', { config }),
  configFactoryReset: (progress: Channel<string>) => call<{ online: boolean; possibleConflicts: number }>('config_factory_reset', { progress }),

  logsHistory: () => call<LogEntry[]>('logs_history', undefined, []),
  logsClear: () => call<void>('logs_clear', undefined, undefined as unknown as void),
  logsExport: (path: string) => call<number>('logs_export', { path }),
  logUi: (level: 'info' | 'warn' | 'error', text: string) => (isTauri ? call<void>('log_ui', { level, text }) : Promise.resolve()),

  coreStatus: () => call<{ tgws: CoreStatus; zapret: CoreStatus }>('core_status', undefined, { tgws: STOPPED, zapret: STOPPED }),
  telegramPowerStatus: () => call<CoreStatus>('telegram_power_status', undefined, STOPPED),
  telegramPower: (on: boolean, id = 0) => call<void>('telegram_power', { on, id }),
  telegramShowLink: () => call<void>('telegram_show_link'),
  tgwsStart: () => call<CoreStatus>('tgws_start'),
  tgwsStop: () => call<CoreStatus>('tgws_stop'),
  tgwsRestart: () => call<CoreStatus>('tgws_restart'),
  tgwsLink: () => call<string>('tgws_link', undefined, `tg://proxy?server=127.0.0.1&port=1443&secret=dd${DEFAULT_CONFIG.tgws.secret}`),
  tgwsRegenerate: () => call<string>('tgws_regenerate_secret'),

  zapretStart: () => call<CoreStatus>('zapret_start'),
  zapretConflicts: () => call<{ conflicts: ZapretConflict[] }>('zapret_conflict_scan', undefined, { conflicts: [] }),
  resolveZapretConflict: (token: string, remember: boolean) => call<CoreStatus>('zapret_conflict_resolve', { token, remember }),
  forgetZapretPermissions: () => call<void>('zapret_conflict_forget'),
  zapretStop: () => call<CoreStatus>('zapret_stop'),
  zapretRestart: () => call<CoreStatus>('zapret_restart'),
  zapretOverview: () => call<ZapretOverview>('zapret_overview', undefined, { strategies: [], fakes: [], ipsetFull: 0, version: '1.10.4', source: 'bundled', ready: false, admin: false }),

  listsInfo: () => call<ListInfo[]>('lists_info', undefined, []),
  listRead: (file: string) => call<string[]>('list_read', { file }, []),
  listWrite: (file: string, entries: string[]) => call<WriteResult>('list_write', { file, entries }),
  listImport: (file: string, path: string) => call<WriteResult>('list_import', { file, path }),
  listExport: (file: string, path: string) => call<number>('list_export', { file, path }),
  listRestoreBackup: (file: string) => call<void>('list_restore_backup', { file }),
  listsFactoryReset: () => call<number>('lists_factory_reset'),
  discordDiagnostics: () => call<DiscordDiagnostics>('discord_diagnostics'),
  discordConfirmMedia: (startedAt: number) => call<DiscordVerified>('discord_confirm_media', { startedAt }),
  discordClearCache: () => call<{ installed: boolean; closed: string[]; removed: number; failed: string[] }>('discord_clear_cache'),

  testerRun: () => call<void>('tester_run'),
  testerCancel: () => call<void>('tester_cancel'),
  testerState: () => call<{ running: boolean; report: TestReport | null }>('tester_state', undefined, { running: false, report: null }),
  testerReset: () => call<void>('tester_reset'),

  releases: () => call<{releases: Release[]; error: string | null}>('updates_zapret_releases', undefined, { releases: [], error: null }),
  siteDomain: (input: string) => call<string>('managed_site_domain', { input }),
  addSite: (input: string) => call<{ domain: string; profile: string | null; state: 'active' | 'stopped' | 'needs_test' }>('managed_site_add', { input }),
  checkSite: (input: string) => call<boolean>('managed_site_check', { input }),
  saveSites: (revision: number, sites: ManagedSite[], services: string[]) => call<{ revision: number; restarted: boolean }>('managed_sites_save', { revision, sites, services }),
  installZapret: (tag: string) => call<{ version: string; strategies: number }>('updates_zapret_install', { tag }),
  checkUpdates: () => call<UpdateCheck>('updates_check'),

  diagCloseListener: (token: string) => call<boolean>('diag_close_listener', { token }, false),
  diagRun: () => call<DiagReport>('diag_run', undefined, { issues: [], passed: [], admin: false, checks: [] }),
  diagOpen: (kind: string) => call<void>('diag_open', { kind }),
};

export async function pickFile(opts: { save?: boolean; defaultPath?: string; title?: string } = {}): Promise<string | null> {
  if (!isTauri) throw new Error(PREVIEW_ERROR);
  const d = await import('@tauri-apps/plugin-dialog');
  const filters = [{ name: 'Текстовый файл', extensions: ['txt'] }];
  if (opts.save) return (await d.save({ title: opts.title, defaultPath: opts.defaultPath, filters })) ?? null;
  const r = await d.open({ title: opts.title, multiple: false, directory: false, filters });
  return typeof r === 'string' ? r : null;
}

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
  }
}
