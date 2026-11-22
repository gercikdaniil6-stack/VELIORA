import { useEffect, useState } from 'react';
import { Icon } from '../components/Icon';
import { Switch } from '../components/bits';
import { Modal, Seg, SelectBox, Spinner } from '../components/ui';
import type { Route } from '../components/Sidebar';
import { api, type ManagedSite } from '../lib/api';
import { useCore } from '../state/core';

type Navigate = (route: Route) => void;
const CATEGORIES = ['Все', 'Нейросети', 'Общение', 'Музыка и видео', 'Другие сервисы'];
const SERVICES = [
  { id: 'openai', title: 'ChatGPT / OpenAI', category: 1, icon: 'sparkle', description: 'ChatGPT и веб-сервисы OpenAI.', host: 'chatgpt.com', aliases: ['chatgpt.com', 'openai.com', 'oaistatic.com', 'oaiusercontent.com'] },
  { id: 'anthropic', title: 'Claude / Anthropic', category: 1, icon: 'sparkle', description: 'Веб-интерфейс Claude и сервисы Anthropic.', host: 'claude.ai', aliases: ['claude.ai', 'anthropic.com'] },
  { id: 'gemini', title: 'Gemini / Google AI', category: 1, icon: 'sparkle', description: 'Gemini и Google AI Studio.', host: 'gemini.google.com', aliases: ['gemini.google.com', 'aistudio.google.com', 'generativelanguage.googleapis.com'] },
  { id: 'grok', title: 'Grok / xAI', category: 1, icon: 'sparkle', description: 'Веб-интерфейс Grok и сервисы xAI.', host: 'grok.com', aliases: ['grok.com', 'x.ai'] },
  { id: 'telegram', title: 'Telegram', category: 2, icon: 'telegram', description: 'Веб-версия и ссылки Telegram.', host: 'telegram.org', aliases: ['telegram.org', 't.me'] },
  { id: 'soundcloud', title: 'SoundCloud', category: 3, icon: 'bolt', description: 'Страницы, музыка и обложки.', host: 'soundcloud.com', aliases: ['soundcloud.com', 'sndcdn.com'] },
  { id: 'twitch', title: 'Twitch', category: 3, icon: 'monitor', description: 'Страницы каналов и видеотрансляции.', host: 'twitch.tv', aliases: ['twitch.tv', 'ttvnw.net', 'jtvnw.net'] },
  { id: 'tiktok', title: 'TikTok', category: 3, icon: 'monitor', description: 'Веб-версия, видео и профили.', host: 'tiktok.com', aliases: ['tiktok.com', 'tiktokcdn.com'] },
  { id: 'github', title: 'GitHub', category: 4, icon: 'terminal', description: 'Репозитории, файлы и материалы GitHub.', host: 'github.com', aliases: ['github.com', 'githubusercontent.com', 'githubassets.com'] },
];

function Search({ value, onChange, label }: { value: string; onChange: (value: string) => void; label: string }) {
  return <div className="manager-search"><Icon name="search" /><input type="search" aria-label={label} placeholder={label} value={value} onChange={(e) => onChange(e.target.value)} /></div>;
}

const PAGE_SIZE = 48;
type View = 'all' | 'ready' | 'mine';
const TABS = [{ id: 'all', label: 'Все сервисы' }, { id: 'ready', label: 'Готовые обходы' }, { id: 'mine', label: 'Мои сайты' }] as const;

export function BypassCatalog({ onNavigate, addOnOpen, onAddOpened }: { onNavigate: Navigate; addOnOpen: boolean; onAddOpened: () => void }) {
  const { config, loaded, status, pending, tester, saveManaged, info, toggle: toggleCore } = useCore();
  const { managedSites = [], managedServices = [], strategy } = config.zapret;
  const [view, setView] = useState<View>('all');
  const [category, setCategory] = useState(0);
  const [query, setQuery] = useState('');
  const [enabledOnly, setEnabledOnly] = useState(false);
  const [saving, setSaving] = useState(false);
  const [details, setDetails] = useState<(typeof SERVICES)[number] | null>(null);
  const [open, setOpen] = useState(addOnOpen);
  const [editor, setEditor] = useState({ domain: null as string | null, name: '', text: '', strategy: '' });
  const [added, setAdded] = useState<Awaited<ReturnType<typeof api.addSite>>>();
  const [check, setCheck] = useState<'pending' | 'ok' | 'failed'>();
  const [checkHint, setCheckHint] = useState('');
  const [advanced, setAdvanced] = useState(false);
  const [error, setError] = useState('');
  const [limit, setLimit] = useState(PAGE_SIZE);
  const disabled = !loaded || saving || pending.zapret || tester.running || check === 'pending';

  useEffect(() => { if (addOnOpen) onAddOpened(); }, [addOnOpen, onAddOpened]);
  useEffect(() => { setCheck(undefined); setCheckHint(''); }, [status.zapret.pid, status.zapret.startedAt, status.zapret.state]);

  const report = tester.report;
  const verified = report && report.bundleVersion === info?.zapretInstalled && !report.cancelled
    ? report.best.filter((file) => report.results[file]?.status === 'passed') : [];
  const search = query.trim().toLowerCase();
  const presets = view !== 'mine' ? SERVICES.filter((service) => (!category || service.category === category)
    && (!enabledOnly || managedServices.includes(service.id))
    && [service.title, service.description, service.host, ...service.aliases].some((text) => text.toLowerCase().includes(search))) : [];
  const shownHosts = new Set(presets.flatMap((service) => [service.host, ...service.aliases]));
  const sites = view !== 'ready' && (!category || category === 4) ? managedSites.filter((site) =>
    (!enabledOnly || site.enabled) && !shownHosts.has(site.domain)
    && (site.domain.includes(search) || (site.name ?? '').toLowerCase().includes(search))) : [];

  function switchView(next: View) {
    setView(next);
    setCategory(0);
    setLimit(PAGE_SIZE);
  }

  function start(site?: ManagedSite) {
    setEditor({ domain: site?.domain ?? null, name: site?.name ?? '', text: site?.domain ?? '', strategy: site?.strategy ?? '' });
    setAdvanced(false);
    setError('');
    setOpen(true);
  }

  async function toggle(id: string, enabled: boolean) {
    setAdded(undefined);
    setCheck(undefined);
    setSaving(true);
    try {
      await saveManaged(managedSites, enabled ? [...managedServices, id] : managedServices.filter((service) => service !== id));
    } finally { setSaving(false); }
  }

  async function commit(next: ManagedSite[]) {
    setAdded(undefined);
    setCheck(undefined);
    setSaving(true);
    try { return await saveManaged(next, managedServices); }
    finally { setSaving(false); }
  }

  async function submit() {
    if (disabled || !editor.text.trim()) return;
    setError('');
    setSaving(true);
    try {
      if (editor.domain) {
        const domain = await api.siteDomain(editor.text);
        if (managedSites.some((site) => site.domain === domain && site.domain !== editor.domain)) {
          setError('Этот сайт уже есть в каталоге.');
          return;
        }
        if (await commit(managedSites.map((site) => site.domain === editor.domain
          ? { ...site, domain, name: editor.name.trim() || null, strategy: editor.strategy || null } : site))) {
          setAdded(undefined);
          setOpen(false);
        }
        return;
      }
      const result = await api.addSite(editor.text);
      setAdded(result);
      setCheck(undefined);
      setCheckHint('');
      setCategory(0);
      setQuery(result.profile ? SERVICES.find((service) => service.id === result.profile)?.title ?? '' : result.domain);
      setEnabledOnly(false);
      setLimit(PAGE_SIZE);
      setView(result.profile ? 'ready' : 'mine');
      setOpen(false);
    } catch (failure) {
      setError((failure as Error).message);
    } finally {
      setSaving(false);
    }
  }

  async function checkAdded() {
    if (!added || disabled) return;
    setCheck('pending');
    setCheckHint('');
    try {
      setCheck(await api.checkSite(added.domain) ? 'ok' : 'failed');
    } catch (failure) {
      setCheckHint((failure as Error).message);
      setCheck('failed');
    }
  }

  const running = status.zapret.state === 'running';
  const checked = check === 'ok' && running;
  const checkText = checked ? 'Сайт доступен' : check === 'failed' ? 'Не удалось подтвердить доступность'
    : check === 'pending' ? 'Проверяем доступность…'
    : added?.state === 'needs_test' ? 'Сайт сохранён' : running ? 'Настройки применены' : 'Настройки сохранены';

  return <div className="page">
    <div className="page-head"><div><h1>Каталог обходов</h1><p>Готовые настройки для сервисов и сайты, которые вы добавили.</p></div>
      <button className="btn btn--primary" onClick={() => start()} disabled={disabled}><Icon name="plus" /> Добавить сайт</button></div>
    <Seg label="Раздел каталога" value={view} onChange={switchView} options={TABS.map((tab) => ({ ...tab, value: tab.id }))} />
    <section className="glass card manager-controls">
      <Search label="Поиск по названию или адресу" value={query} onChange={(value) => { setQuery(value); setLimit(PAGE_SIZE); }} />
      <Switch label="Только включённые" checked={enabledOnly} onChange={(value) => { setEnabledOnly(value); setLimit(PAGE_SIZE); }} />
      <span className="muted">Только включённые</span>
    </section>
    {view !== 'mine' && <nav className="manager-filters" aria-label="Категории сервисов">
      {CATEGORIES.map((name, i) => <button key={name} type="button" className={'btn' + (category === i ? ' btn--primary' : '')}
        aria-pressed={category === i} onClick={() => { setCategory(i); setLimit(PAGE_SIZE); }}>{name}</button>)}
    </nav>}
    {added && <section className="glass card manager-controls" role="status">
      <div style={{ flex: 1, minWidth: 200 }}>
        <strong>{checkText}</strong>
        <p className="muted">{added.domain} · {checkHint || (
          checked ? 'Главная страница ответила по HTTPS.' : check === 'failed'
            ? 'Проверьте сайт в браузере или запустите диагностику Zapret.'
            : added.state === 'needs_test' ? 'Подходящие настройки пока не подтверждены. Запустите тест Zapret.'
            : running ? 'Можно проверить, отвечает ли главная страница.' : 'Включите Zapret, чтобы применить настройки.'
        )}</p>
      </div>
      {running && added.state !== 'needs_test' && <button className="btn btn--primary" disabled={disabled} onClick={() => void checkAdded()}>
        {check === 'pending' && <Spinner size={16} />} {check ? 'Проверить снова' : 'Проверить доступность'}
      </button>}
      {!running && added.state !== 'needs_test' && info?.admin && <button className="btn btn--primary" disabled={disabled}
        onClick={() => void toggleCore('zapret', true)}>Включить обход</button>}
      {(check === 'failed' || added.state === 'needs_test' || (!running && !info?.admin)) &&
        <button className="btn" onClick={() => onNavigate('zapret')}>Открыть Zapret</button>}
      <button className="btn btn--ghost btn--icon" aria-label="Закрыть результат" disabled={check === 'pending'} onClick={() => setAdded(undefined)}><Icon name="close" /></button>
    </section>}
    {presets.length > 0 && <div className="glass card manager-presets">
      {presets.map((s) => {
        const enabled = managedServices.includes(s.id);
        const state = !enabled ? 'Выключено' : !strategy ? 'Ожидает настройки' : status.zapret.state === 'running' ? 'Настройки применены' : 'Включится с Zapret';
        return <section className="manager-service" key={s.id}>
          <header><span className="tile__icon"><Icon name={s.icon} /></span><h2>{s.title}</h2><button className="btn btn--ghost btn--icon" aria-label={`Настройки ${s.title}`} onClick={() => setDetails(s)}><Icon name="settings" size={16} /></button><Switch checked={enabled} disabled={disabled} label={s.title} onChange={(v) => void toggle(s.id, v)} /></header>
          <div className="manager-service__meta"><span className={`chip${enabled ? ' chip--ok' : ''}`}>{state}</span><span className="muted">Сайт ещё не проверен</span></div>
        </section>;
      })}
    </div>}
    {sites.length > 0 && <section className="glass card manager-site-list">
      {sites.slice(0, limit).map((site) => <div className="manager-site" key={site.domain}>
        <span className="tile__icon"><Icon name="globe" /></span>
        <div><strong>{site.name || site.domain}</strong>{site.name && <span className="managed-domain muted">{site.domain}</span>}<p className="muted">{
          !site.enabled ? 'Выключен' : site.strategy && site.strategy !== strategy ? 'Ожидает подходящих настроек' : status.zapret.state === 'running' ? 'В активном списке' : 'Включится с Zapret'
        }</p></div>
        <Switch checked={site.enabled} label={`Включить ${site.domain}`} disabled={disabled} onChange={(enabled) => void commit(managedSites.map((s) => s.domain === site.domain ? { ...s, enabled } : s))} />
        <button className="btn btn--ghost btn--icon" aria-label={`Изменить ${site.domain}`} disabled={disabled} onClick={() => start(site)}><Icon name="edit" /></button>
        <button className="btn btn--ghost btn--icon" aria-label={`Удалить ${site.domain}`} disabled={disabled} onClick={() => void commit(managedSites.filter((s) => s.domain !== site.domain))}><Icon name="trash" /></button>
      </div>)}
      {sites.length > limit && <button className="btn" onClick={() => setLimit((value) => value + PAGE_SIZE)}>Показать ещё</button>}
    </section>}
    {!presets.length && !sites.length && <p className="muted">{view === 'mine' && !managedSites.length ? 'Добавьте первый сайт по ссылке.' : 'Совпадений не найдено.'}</p>}
    <Modal open={details !== null} onClose={() => setDetails(null)} title={details?.title ?? ''} lead={details?.description}
      foot={<><button className="btn" onClick={() => { setDetails(null); onNavigate('zapret'); }}>Стратегии Zapret</button>
        {details?.id === 'telegram' && <button className="btn" onClick={() => { setDetails(null); onNavigate('telegram'); }}>Telegram-прокси</button>}</>}>
      <div className="stack">
        <div className="setting"><div><b>Добавить домены к стратегии</b><p className="muted">Правило дополняет списки стратегии. При выключении базовые правила сборки по-прежнему действуют.</p></div>
          {details && <Switch checked={managedServices.includes(details.id)} disabled={disabled} label={details.title} onChange={(v) => void toggle(details.id, v)} />}</div>
        <p className="muted">Основной домен · {details?.host}</p>
      </div>
    </Modal>
    <Modal open={open} onClose={() => { if (!saving) setOpen(false); }} title={editor.domain ? 'Редактировать сайт' : 'Добавить сайт'}
      foot={<><button className="btn" disabled={saving} onClick={() => setOpen(false)}>Отмена</button>
        <button className="btn btn--primary" type="submit" form="catalog-site-form" disabled={disabled || !editor.text.trim()}>
          {saving && <Spinner size={16} />} {editor.domain ? 'Сохранить' : 'Добавить и настроить'}</button></>}>
      <form id="catalog-site-form" className="stack" onSubmit={(event) => { event.preventDefault(); void submit(); }}>
        <label className="field">
          <span>Вставьте ссылку на сайт</span>
          <input className="input" inputMode="url" autoComplete="off" spellCheck={false} maxLength={2048}
            value={editor.text} placeholder="https://example.com"
            disabled={saving} onChange={(event) => { setError(''); setEditor({ ...editor, text: event.target.value }); }}
            onPaste={(event) => {
              if (/\s/.test(event.clipboardData.getData('text').trim())) {
                event.preventDefault();
                setError('Вставьте одну ссылку на сайт.');
              }
            }} />
        </label>
        <p className="muted">Например: <code>https://example.com</code></p>
        {saving && <p role="status" className="muted">Настраиваем обход…</p>}
        {editor.domain && <>
          <button type="button" className="btn btn--ghost" aria-expanded={advanced} onClick={() => setAdvanced(!advanced)}>Дополнительные настройки</button>
          {advanced && <>
            <label className="field">Название сайта
              <input className="input" maxLength={80} value={editor.name} onChange={(event) => setEditor({ ...editor, name: event.target.value })} />
            </label>
            <SelectBox label="Стратегия сайта" disabled={saving} value={editor.strategy}
              options={[{ value: '', label: 'Любая активная стратегия' }, ...verified.map((file) => ({ value: file, label: file.replace(/\.bat$/i, '') + ' · прошла тест' })),
                ...(editor.strategy && !verified.includes(editor.strategy) ? [{ value: editor.strategy, label: editor.strategy + ' · сохранённая' }] : [])]}
              onChange={(value) => setEditor({ ...editor, strategy: value })} />
          </>}
        </>}
        {error && <div role="alert"><p className="field__error">{error}</p>{error.includes('Zapret') && <button className="btn btn--ghost" type="button" onClick={() => onNavigate('zapret')}>Открыть Zapret</button>}</div>}
      </form>
    </Modal>
  </div>;
}
