import { useEffect, useState } from 'react';
import { BrandLogo } from '../brand/BrandLogo';
import { Icon } from '../components/Icon';
import { LatencyBadge, formatDuration, formatMs, phaseTitle } from '../components/bits';
import { useStore } from '../state/store';

export function Overview({ onNavigate }: { onNavigate: (r: 'servers') => void }) {
  const { data, conn, latency, active, connect, disconnect, probe, setActive } = useStore();
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (conn.phase !== 'connected') return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [conn.phase]);

  const current = data.profiles.find((p) => p.id === conn.profileId) ?? active;
  const busy = conn.phase === 'connecting' || conn.phase === 'disconnecting';
  const on = conn.phase === 'connected';
  const lat = current ? latency[current.id] : undefined;

  if (!data.profiles.length) {
    return (
      <div className="page">
        <Head />
        <div className="glass empty" style={{ borderRadius: 'var(--r-xl)' }}>
          <BrandLogo variant="symbol" size={56} decorative />
          <h3>Добавьте первый сервер</h3>
          <p>VELIORA направит трафик Windows через выбранный прокси-сервер и вернёт прежние настройки при отключении.</p>
          <button className="btn btn--primary btn--lg" onClick={() => onNavigate('servers')}><Icon name="plus" /> Добавить сервер</button>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <Head />

      <section className="glass glass--strong hero" aria-live="polite">
        <div>
          <div className="eyebrow">Соединение</div>
          <div className={`hero__status${on ? ' hero__status--on' : ''}`}>{phaseTitle(conn.phase)}</div>
          <div className="hero__server">
            {current && <>
              <span>{current.name}</span>
              <span className="chip">{current.kind === 'socks5' ? 'SOCKS' : 'HTTP'}</span>
              <span className="mono muted">{current.host}:{current.port}</span>
            </>}
          </div>
          <div className="hero__actions">
            {on || conn.phase === 'disconnecting'
              ? <button className="btn btn--lg" disabled={busy} onClick={disconnect}><Icon name="power" /> Отключить</button>
              : <button className="btn btn--primary btn--lg" disabled={busy} onClick={() => connect(current)}><Icon name="bolt" /> Подключить</button>}
            <button className="btn btn--lg btn--ghost" onClick={() => onNavigate('servers')}>Сменить сервер</button>
          </div>
          {conn.phase === 'error' && <div className="hero__error">{conn.error}</div>}
        </div>

        <div className="route" data-phase={conn.phase}>
          <div className="route__line" />
          <div className="route__pulse" />
          <div className="route__node">
            <div className="route__orb"><Icon name="device" size={22} /></div>
            <div className="route__label"><b>Это устройство</b>Windows</div>
          </div>
          <div className="route__mark"><BrandLogo variant="symbol" size={30} decorative /></div>
          <div className="route__node">
            <div className="route__orb"><Icon name="globe" size={22} /></div>
            <div className="route__label"><b>{current?.name ?? 'Сервер'}</b>{current?.kind === 'socks5' ? 'SOCKS' : 'HTTP'}-прокси</div>
          </div>
        </div>
      </section>

      <div className="grid grid--4 section-gap">
        <div className="glass stat">
          <div className="eyebrow">Задержка</div>
          <div className="stat__value">{lat?.ms != null ? formatMs(lat.ms) : '—'}</div>
          <div className="stat__hint">{lat?.error ? 'Сервер не отвечает' : 'TCP-рукопожатие'}</div>
        </div>
        <div className="glass stat">
          <div className="eyebrow">Сеанс</div>
          <div className="stat__value">{on && conn.since ? formatDuration(now - conn.since) : '00:00'}</div>
          <div className="stat__hint">{on ? 'Активное подключение' : 'Нет активного сеанса'}</div>
        </div>
        <div className="glass stat">
          <div className="eyebrow">Режим</div>
          <div className="stat__value">Системный</div>
          <div className="stat__hint">Прокси Windows (WinINet)</div>
        </div>
        <div className="glass stat">
          <div className="eyebrow">Исключения</div>
          <div className="stat__value">{data.rules.bypass.length + (data.rules.bypassLocal ? 1 : 0)}</div>
          <div className="stat__hint">Адресов в обход прокси</div>
        </div>
      </div>

      <section className="glass card section-gap">
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
          <h2>Быстрый выбор</h2>
          <button className="btn btn--ghost" onClick={() => onNavigate('servers')}>Все серверы <Icon name="arrow" size={16} /></button>
        </div>
        <div className="list">
          {data.profiles.slice(0, 5).map((p) => {
            const isActive = p.id === active?.id;
            return (
              <div key={p.id} className={`row${isActive ? ' row--active' : ''}`}>
                <div style={{ minWidth: 0 }}>
                  <div className="row__name"><span>{p.name}</span>{isActive && <span className="chip chip--accent">Активен</span>}</div>
                  <div className="row__sub mono">{p.host}:{p.port}</div>
                </div>
                <span className="chip" style={{ justifySelf: 'start' }}>{p.kind === 'socks5' ? 'SOCKS' : 'HTTP'}</span>
                <LatencyBadge v={latency[p.id]} />
                <div className="row__actions">
                  <button className="btn btn--icon btn--ghost" title="Проверить задержку" aria-label={`Проверить задержку: ${p.name}`} onClick={() => probe(p)}><Icon name="refresh" size={16} /></button>
                  <button className="btn" disabled={isActive} onClick={() => setActive(p.id)}>{isActive ? <><Icon name="check" size={16} /> Выбран</> : 'Выбрать'}</button>
                </div>
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}

function Head() {
  return (
    <div className="page-head">
      <div>
        <h1>Системный прокси</h1>
        <p>Направляет трафик Windows через ваш прокси-сервер и возвращает прежние настройки при отключении.</p>
      </div>
    </div>
  );
}
