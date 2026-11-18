import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Icon } from '../components/Icon';
import { LatencyBadge } from '../components/bits';
import { uid, useStore } from '../state/store';
import type { Profile, ProxyKind } from '../state/types';

export function Servers() {
  const { data, latency, active, conn, update, probe, probeAll, setActive } = useStore();
  const [editing, setEditing] = useState<Profile | 'new' | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  const remove = (p: Profile) => {
    if (conn.phase === 'connected' && conn.profileId === p.id) return;
    setConfirming(null);
    update((d) => ({
      ...d,
      profiles: d.profiles.filter((x) => x.id !== p.id),
      activeId: d.activeId === p.id ? undefined : d.activeId,
    }));
  };

  const save = (p: Profile) => {
    update((d) => {
      const exists = d.profiles.some((x) => x.id === p.id);
      return {
        ...d,
        profiles: exists ? d.profiles.map((x) => (x.id === p.id ? p : x)) : [...d.profiles, p],
        activeId: d.activeId ?? p.id,
      };
    });
    setEditing(null);
    probe(p);
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Серверы</h1>
          <p>Профили прокси-серверов. Активный профиль используется для подключения и в меню трея.</p>
        </div>
        <div className="page-head__actions">
          <button className="btn" disabled={!data.profiles.length} onClick={() => probeAll()}><Icon name="refresh" size={16} /> Проверить все</button>
          <button className="btn btn--primary" onClick={() => setEditing('new')}><Icon name="plus" size={16} /> Добавить</button>
        </div>
      </div>

      <section className="glass card">
        {data.profiles.length === 0 ? (
          <div className="empty">
            <h3>Серверов пока нет</h3>
            <p>Добавьте HTTP или SOCKS прокси: адрес и порт можно взять у вашего провайдера.</p>
            <button className="btn btn--primary" onClick={() => setEditing('new')}><Icon name="plus" size={16} /> Добавить сервер</button>
          </div>
        ) : (
          <div className="list" style={{ ['--row-actions' as string]: '290px' }}>
            {data.profiles.map((p) => {
              const isActive = p.id === active?.id;
              const live = conn.phase === 'connected' && conn.profileId === p.id;
              return (
                <div key={p.id} className={`row${isActive ? ' row--active' : ''}`}>
                  <div style={{ minWidth: 0 }}>
                    <div className="row__name">
                      <span>{p.name}</span>
                      {live ? <span className="chip chip--accent">Подключён</span> : isActive && <span className="chip chip--accent">Активен</span>}
                    </div>
                    <div className="row__sub"><span className="mono">{p.host}:{p.port}</span>{p.note ? ` · ${p.note}` : ''}</div>
                  </div>
                  <span className="chip" style={{ justifySelf: 'start' }}>{p.kind === 'socks5' ? 'SOCKS' : 'HTTP'}</span>
                  <LatencyBadge v={latency[p.id]} />
                  {confirming === p.id ? (
                  <div className="row__actions">
                    <span className="muted" style={{ fontSize: 13, alignSelf: 'center', marginRight: 4 }}>Удалить сервер?</span>
                    <button className="btn btn--ghost" onClick={() => setConfirming(null)}>Отмена</button>
                    <button className="btn" style={{ color: 'var(--bad)' }} onClick={() => remove(p)}>Удалить</button>
                  </div>
                  ) : (
                  <div className="row__actions">
                    {!isActive && <button className="btn" onClick={() => setActive(p.id)}>Сделать активным</button>}
                    <button className="btn btn--icon btn--ghost" title="Проверить" aria-label={`Проверить ${p.name}`} onClick={() => probe(p)}><Icon name="refresh" size={16} /></button>
                    <button className="btn btn--icon btn--ghost" title="Изменить" aria-label={`Изменить ${p.name}`} onClick={() => setEditing(p)}><Icon name="edit" size={16} /></button>
                    <button className="btn btn--icon btn--ghost btn--danger" title={live ? 'Сначала отключитесь' : 'Удалить'} aria-label={`Удалить ${p.name}`} disabled={live} onClick={() => setConfirming(p.id)}><Icon name="trash" size={16} /></button>
                  </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </section>

      {editing && <ProfileEditor initial={editing === 'new' ? undefined : editing} onSave={save} onClose={() => setEditing(null)} />}
    </div>
  );
}

const HOST_RE = /^(?=.{1,253}$)([a-zA-Z0-9-]{1,63}\.)*[a-zA-Z0-9-]{1,63}$|^\[?[0-9a-fA-F:]+\]?$/;

function ProfileEditor({ initial, onSave, onClose }: { initial?: Profile; onSave: (p: Profile) => void; onClose: () => void }) {
  const [name, setName] = useState(initial?.name ?? '');
  const [kind, setKind] = useState<ProxyKind>(initial?.kind ?? 'http');
  const [host, setHost] = useState(initial?.host ?? '');
  const [port, setPort] = useState(initial ? String(initial.port) : '');
  const [note, setNote] = useState(initial?.note ?? '');
  const [touched, setTouched] = useState(false);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => {
    first.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const portN = Number(port);
  const errors = {
    host: !host.trim() ? 'Укажите адрес' : !HOST_RE.test(host.trim()) ? 'Адрес без протокола и пробелов, например 203.0.113.5 или proxy.example.com' : '',
    port: !port ? 'Укажите порт' : !Number.isInteger(portN) || portN < 1 || portN > 65535 ? 'Порт от 1 до 65535' : '',
  };
  const valid = !errors.host && !errors.port;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (!valid) return;
    onSave({
      id: initial?.id ?? uid(),
      name: name.trim() || host.trim(),
      kind,
      host: host.trim(),
      port: portN,
      note: note.trim() || undefined,
    });
  };

  return createPortal(
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <form className="glass glass--strong sheet" role="dialog" aria-modal="true" aria-labelledby="pe-title" onSubmit={submit}>
        <h2 id="pe-title">{initial ? 'Изменить сервер' : 'Новый сервер'}</h2>
        <div className="muted" style={{ fontSize: 13 }}>Системный прокси Windows не поддерживает логин и пароль — используйте серверы с доступом по IP.</div>
        <div className="sheet__body">
          <label className="field">
            <span className="field__label">Название</span>
            <input ref={first} className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Например, Франкфурт" maxLength={48} />
          </label>
          <div className="field">
            <span className="field__label">Протокол</span>
            <div className="segmented" role="group" aria-label="Протокол">
              <button type="button" aria-pressed={kind === 'http'} onClick={() => setKind('http')}>HTTP / HTTPS</button>
              <button type="button" aria-pressed={kind === 'socks5'} onClick={() => setKind('socks5')}>SOCKS</button>
            </div>
          </div>
          <div className="form-grid">
            <label className="field">
              <span className="field__label">Адрес</span>
              <input className="input mono" value={host} onChange={(e) => setHost(e.target.value)} placeholder="203.0.113.5" aria-invalid={touched && !!errors.host} spellCheck={false} />
              {touched && errors.host && <span className="field__error">{errors.host}</span>}
            </label>
            <label className="field">
              <span className="field__label">Порт</span>
              <input className="input mono" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ''))} placeholder="8080" inputMode="numeric" aria-invalid={touched && !!errors.port} />
              {touched && errors.port && <span className="field__error">{errors.port}</span>}
            </label>
          </div>
          <label className="field">
            <span className="field__label">Заметка <span style={{ color: 'var(--text-3)', fontWeight: 500 }}>— необязательно</span></span>
            <input className="input" value={note} onChange={(e) => setNote(e.target.value)} placeholder="Рабочий, резервный…" maxLength={64} />
          </label>
        </div>
        <div className="sheet__foot">
          <button type="button" className="btn btn--ghost" onClick={onClose}>Отмена</button>
          <button type="submit" className="btn btn--primary">{initial ? 'Сохранить' : 'Добавить сервер'}</button>
        </div>
      </form>
    </div>,
    document.body,
  );
}
