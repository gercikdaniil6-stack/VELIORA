import { BrandLogo } from '../brand/BrandLogo';
import { BRAND } from '../brand/brand';
import { Icon } from '../components/Icon';
import { api } from '../lib/api';
import { useCore } from '../state/core';

const SOURCES = [
  { name: 'Flowseal / tg-ws-proxy', use: 'Telegram WS-прокси (собран из исходников как консольная программа)', license: 'MIT', url: 'https://github.com/Flowseal/tg-ws-proxy' },
  { name: 'Flowseal / zapret-discord-youtube', use: 'Стратегии и списки Zapret', license: 'MIT', url: 'https://github.com/Flowseal/zapret-discord-youtube' },
  { name: 'bol-van / zapret', use: 'Движок обхода DPI winws', license: 'MIT', url: 'https://github.com/bol-van/zapret' },
  { name: 'WinDivert', use: 'Перехват сетевых пакетов в Windows', license: 'LGPL-3.0 / GPL-2.0', url: 'https://reqrypt.org/windivert.html' },
];

export function About() {
  const { info } = useCore();
  const v = info?.version ?? '—';

  return (
    <div className="page">
      <section className="glass glass--strong about-hero">
        <BrandLogo variant="lockup" size={92} />
        <div className="about-hero__name">{BRAND.product} · версия {v}</div>
        <p>
          Telegram и свободный интернет в одном окне: локальный Telegram-прокси, обход DPI с подбором стратегий под вашу сеть
          и системный прокси Windows. Две линии сходятся в одну точку — так устроен знак VELIORA и так работает соединение.
        </p>
      </section>

      <div className="grid grid--2 section-gap" style={{ alignItems: 'start' }}>
        <section className="glass card">
          <h2 style={{ marginBottom: 16 }}>Сведения</h2>
          <dl className="kv">
            <dt>Версия</dt><dd className="mono">{v}</dd>
            <dt>Telegram-прокси</dt><dd className="mono">TgWsProxy {info?.tgwsVersion ?? '—'}</dd>
            <dt>Сборка Zapret</dt><dd className="mono">{info?.zapretInstalled ?? '—'}{info?.zapretSource === 'update' ? ' (обновлена)' : ''}</dd>
            <dt>Права</dt><dd>{info ? (info.admin ? 'Администратор' : 'Обычные (Zapret недоступен)') : '—'}</dd>
            <dt>Данные</dt><dd className="mono wrap">{info?.dataDir || '—'}</dd>
            <dt>Файлы Zapret</dt><dd className="mono wrap">{info?.zapretDir || '—'}</dd>
            <dt>Платформа</dt><dd>Tauri 2 · Rust · React · TypeScript</dd>
            <dt>Правообладатель</dt><dd>© 2026 {BRAND.name}</dd>
          </dl>
        </section>

        <section className="glass card">
          <h2>Открытые компоненты</h2>
          <p className="lead">VELIORA App запускает эти программы как отдельные процессы и благодарит их авторов.</p>
          <div className="credits">
            {SOURCES.map((s) => (
              <button key={s.name} className="credit" onClick={() => api.openUrl(s.url).catch(() => {})}>
                <span className="credit__name">{s.name} <span className="chip">{s.license}</span></span>
                <span className="credit__use">{s.use}</span>
                <Icon name="external" size={14} />
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
