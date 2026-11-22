import { useEffect, useState } from 'react';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Banner, Field, Seg, ServiceCard, Spinner } from '../components/ui';
import { Setting } from '../components/bits';
import { api, copyText } from '../lib/api';
import { useCore } from '../state/core';

const HOST_RE = /^(?:(?:\d{1,3}\.){3}\d{1,3}|(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*)$/i;
const DOMAIN_RE = /^(?!\d+\.\d+\.\d+\.\d+$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9-]{2,63}$/i;

import { TelegramProxy } from './TelegramProxy';

const modes = [{ value: 'catalog', label: 'Серверы' }, { value: 'local', label: 'Локальный режим' }];

export function Telegram() {
  const [mode, setMode] = useState('catalog');
  return <div className="page">
    <header className="page-head"><div><h1>Telegram</h1><p>Подключение через прокси и выбор сервера.</p></div></header>
    <div style={{ marginBottom: 'var(--s-5)' }}><Seg label="Режим Telegram" value={mode} options={modes} onChange={setMode} /></div>
    {mode === 'catalog' ? <TelegramProxy /> : <LocalProxy />}
  </div>;
}

function LocalProxy() {
  const core = useCore();
  const toast = useToast();
  const t = core.config.tgws;
  const st = core.status.tgws;
  const [link, setLink] = useState('');
  const [showLink, setShowLink] = useState(false);
  const [showSecret, setShowSecret] = useState(false);
  const [regen, setRegen] = useState(false);
  const [form, setForm] = useState({ host: t.host, port: String(t.port), secret: t.secret, worker: t.workerDomain });

  useEffect(() => { setForm({ host: t.host, port: String(t.port), secret: t.secret, worker: t.workerDomain }); }, [t.host, t.port, t.secret, t.workerDomain]);
  useEffect(() => { api.tgwsLink().then(setLink).catch(() => setLink('')); }, [t.host, t.port, t.secret]);

  const errors = {
    host: HOST_RE.test(form.host.trim()) ? null : 'Хост должен быть IP-адресом или доменным именем.',
    port: /^\d+$/.test(form.port) && +form.port >= 1 && +form.port <= 65535 ? null : 'Порт — число от 1 до 65535.',
    secret: /^[0-9a-f]{32}$/i.test(form.secret.trim()) ? null : 'Ключ — ровно 32 шестнадцатеричных символа (0–9, a–f).',
    worker: !form.worker.trim() || DOMAIN_RE.test(form.worker.trim().replace(/^https?:\/\//, '').replace(/\/$/, '')) ? null : 'Нужен домен без https:// и пути, например name.user.workers.dev.',
  };
  const dirty = form.host.trim() !== t.host || +form.port !== t.port || form.secret.trim().toLowerCase() !== t.secret || form.worker.trim() !== t.workerDomain;
  const valid = !errors.host && !errors.port && !errors.secret && !errors.worker;
  const addressChanged = form.host.trim() !== t.host || +form.port !== t.port || form.secret.trim().toLowerCase() !== t.secret;

  const saveParams = async () => {
    const ok = await core.save((c) => ({
      ...c,
      tgws: { ...c.tgws, host: form.host.trim(), port: +form.port, secret: form.secret.trim().toLowerCase(), workerDomain: form.worker.trim().replace(/^https?:\/\//, '').replace(/\/$/, '') },
    }));
    if (ok && addressChanged) toast.show({ title: 'Обновите ссылку в Telegram', body: 'Адрес, порт или ключ изменились — добавьте прокси в Telegram заново по новой ссылке.' });
  };

  const copy = async () => {
    await copyText(link);
    toast.show({ tone: 'ok', title: 'Ссылка скопирована' });
  };
  const open = () => api.openUrl(link).catch((e) => toast.show({ tone: 'error', title: 'Не удалось открыть Telegram', body: e.message }));
  const regenerate = async () => {
    setRegen(true);
    try {
      setLink(await api.tgwsRegenerate());
      toast.show({ tone: 'ok', title: 'Ключ и ссылка изменены', body: 'Подключите прокси в Telegram заново по новой ссылке.' });
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не удалось сменить ключ', body: (e as Error).message });
    } finally {
      setRegen(false);
    }
  };

  const masked = link.replace(/secret=dd([0-9a-f]{4})[0-9a-f]+/i, 'secret=dd$1••••••••••••••••••••••••••••');

  return (
    <>
      <div className="stack">
        <ServiceCard icon="telegram" title="Локальный прокси" subtitle={<span className="mono">{t.host}:{t.port}</span>} status={st} pending={core.pending.tgws}
          version={core.info?.tgwsVersion} onToggle={(on) => core.toggle('tgws', on)}>
          <span className="service__note">{st.state === 'running' ? 'Telegram может подключаться' : 'Нажмите переключатель, чтобы запустить'}</span>
        </ServiceCard>

        <section className="glass card">
          <h2>Подключение</h2>
          <p className="lead">Добавьте эту ссылку в Telegram один раз — она не меняется между перезапусками.</p>
          <div className="linkbox">
            <code className="linkbox__value mono" title={showLink ? link : undefined}>{showLink ? link : masked}</code>
            <button className="btn btn--icon" aria-label={showLink ? 'Скрыть ссылку' : 'Показать ссылку'} title={showLink ? 'Скрыть ссылку' : 'Показать ссылку'} onClick={() => setShowLink(!showLink)}><Icon name={showLink ? 'eyeOff' : 'eye'} size={16} /></button>
            <button className="btn btn--icon" aria-label="Копировать ссылку" title="Копировать ссылку" onClick={copy} disabled={!link}><Icon name="copy" size={16} /></button>
            <button className="btn btn--icon" aria-label="Сменить ключ и ссылку" title="Сменить ключ и ссылку" onClick={regenerate} disabled={regen}>{regen ? <Spinner size={14} /> : <Icon name="key" size={16} />}</button>
            <button className="btn btn--primary" onClick={open} disabled={!link}><Icon name="external" size={16} /> Открыть в Telegram</button>
          </div>
          <p className="field__hint" style={{ marginTop: 12 }}>
            «Открыть в Telegram» добавит прокси автоматически. Вручную: Telegram → Настройки → Продвинутые настройки → Тип соединения → «Использовать собственный прокси» → вставьте ссылку.
          </p>
        </section>

        <section className="glass card">
          <h2>Параметры</h2>
          <p className="lead">Изменения применяются кнопкой «Сохранить». Если прокси работает, он перезапустится сам.</p>
          <div className="form-grid form-grid--tg">
            <Field label="Хост" error={errors.host} hint="127.0.0.1 — прокси доступен только на этом компьютере.">
              <input className="input" value={form.host} aria-invalid={!!errors.host} onChange={(e) => setForm({ ...form, host: e.target.value })} spellCheck={false} />
            </Field>
            <Field label="Порт" error={errors.port}>
              <input className="input" inputMode="numeric" value={form.port} aria-invalid={!!errors.port} onChange={(e) => setForm({ ...form, port: e.target.value.replace(/\D/g, '').slice(0, 5) })} />
            </Field>
          </div>
          <div className="stack" style={{ gap: 16, marginTop: 16 }}>
            <Field label="Секретный ключ (32 hex)" error={errors.secret}>
              <div className="input-row">
                <input className="input mono" type={showSecret ? 'text' : 'password'} value={form.secret} aria-invalid={!!errors.secret}
                  onChange={(e) => setForm({ ...form, secret: e.target.value.trim() })} spellCheck={false} autoComplete="off" />
                <button className="btn btn--icon" aria-label={showSecret ? 'Скрыть ключ' : 'Показать ключ'} onClick={() => setShowSecret(!showSecret)}><Icon name={showSecret ? 'eyeOff' : 'eye'} size={16} /></button>
              </div>
            </Field>
            <Field label="Свой Cloudflare Worker (необязательно)" error={errors.worker}
              hint="Свой адрес вместо общих узлов Cloudflare. Может помочь с загрузкой медиа и стикеров. Укажите домен воркера, например name.user.workers.dev.">
              <input className="input mono" placeholder="name.user.workers.dev" value={form.worker} aria-invalid={!!errors.worker} onChange={(e) => setForm({ ...form, worker: e.target.value })} spellCheck={false} />
            </Field>
          </div>
          {dirty && addressChanged && <Banner tone="info" icon="info" title="После сохранения ссылка изменится">Её нужно будет заново добавить в Telegram.</Banner>}
          <div className="card__foot">
            <button className="btn btn--ghost" disabled={!dirty} onClick={() => setForm({ host: t.host, port: String(t.port), secret: t.secret, worker: t.workerDomain })}>Отменить</button>
            <button className="btn btn--primary" disabled={!dirty || !valid} onClick={saveParams}>Сохранить</button>
          </div>
        </section>

        <section className="glass card">
          <h2>Запуск</h2>
          <Setting title="Включать вместе с VELIORA App" desc="Прокси запускается сразу при старте приложения." checked={t.autoStart}
            onChange={(v) => core.save((c) => ({ ...c, tgws: { ...c.tgws, autoStart: v } }), { quiet: true })} />
          <Setting title="Подробный журнал" desc="Записывает каждое соединение в «Журнал» — для поиска причин сбоев." checked={t.verbose}
            onChange={(v) => core.save((c) => ({ ...c, tgws: { ...c.tgws, verbose: v } }), { quiet: true })} />
        </section>
      </div>
    </>
  );
}
