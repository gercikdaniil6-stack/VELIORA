export type ProxyKind = 'http' | 'socks5';

export interface Profile {
  id: string;
  name: string;
  kind: ProxyKind;
  host: string;
  port: number;
  note?: string;
}

export interface Rules {
  bypass: string[];
  bypassLocal: boolean;
}

export interface Automation {
  connectOnLaunch: boolean;
  reapplyOnSwitch: boolean;
  autoProbe: boolean;
}

export interface Preferences {
  closeToTray: boolean;
  showSplash: boolean;
  reduceTransparency: boolean;
}

export type Phase = 'idle' | 'connecting' | 'connected' | 'disconnecting' | 'error';

export interface Connection {
  phase: Phase;
  since?: number;
  server?: string;
  profileId?: string;
  error?: string;
}

export interface PersistedState {
  version: 1;
  profiles: Profile[];
  activeId?: string;
  rules: Rules;
  automation: Automation;
  prefs: Preferences;
}

export type Latency = Record<string, { ms?: number; error?: string; pending?: boolean; at?: number }>;
