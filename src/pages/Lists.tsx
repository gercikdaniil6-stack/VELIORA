import { useCallback, useEffect, useMemo, useState } from 'react';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Banner, Confirm, Spinner } from '../components/ui';
import { api, type ListInfo } from '../lib/api';
import { useCore } from '../state/core';

const DOMAIN_RE = /^\^?(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9-]{2,63}$/i;
const IP_RE = /^(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\/(?:3[0-2]|[12]?\d))?$|^[0-9a-f:]+:[0-9a-f:]*(?:\/\d{1,3})?$/i;
const plural = (n: number, one: string, few: string, many: string) => {
  const m10 = n % 10, m100 = n % 100;
  return `${n} ${m10 === 1 && m100 !== 11 ? one : m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14) ? few : many}`;
};
const count = (kind: string, n: number) => (kind === 'ips' ? plural(n, 'адрес', 'адреса', 'адресов') : plural(n, 'домен', 'домена', 'доменов'));

export function Lists() {
  const core = useCore();
  const toast = useToast();
  const [lists, setLists] = useState<ListInfo[]>([]);
  const [file, setFile] = useState<string>('list-general-user.txt');
  const [saved, setSaved] = useState<string[]>([]);
  const [entries, setEntries] = useState<string[]>([]);
  const [past, setPast] = useState<string[][]>([]);
  const [future, setFuture] = useState<string[][]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [draft, setDraft] = useState('');
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<null | 'factory' | 'switch' | 'backup'>(null);
  const [nextFile, setNextFile] = useState<string | null>(null);
  const admin = core.info?.admin ?? false;
  const locked = !admin || core.tester.running;

  const info = lists.find((l) => l.file === file);
  const kind = info?.kind ?? 'domains';
  const dirty = JSON.stringify(entries) !== JSON.stringify(saved);

  const refreshInfo = useCallback(() => api.listsInfo().then(setLists).catch(() => {}), []);
  const loadFile = useCallback(async (f: string) => {
    const e = await api.listRead(f).catch(() => [] as string[]);
    setSaved(e); setEntries(e); setPast([]); setFuture([]); setSelected(new Set());
  }, []);
  useEffect(() => { refreshInfo(); }, [refreshInfo]);
  useEffect(() => { loadFile(file); }, [file, loadFile]);

  const commit = (next: string[]) => { setPast((p) => [...p.slice(-50), entries]); setFuture([]); setEntries(next); };
  const undo = () => { if (!past.length) return; setFuture((f) => [entries, ...f]); setEntries(past[past.length - 1]); setPast((p) => p.slice(0, -1)); };
  const redo = () => { if (!future.length) return; setPast((p) => [...p, entries]); setEntries(future[0]); setFuture((f) => f.slice(1)); };

  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === 'TEXTAREA' || (e.target as HTMLElement)?.tagName === 'INPUT') return;
      if (e.ctrlKey && e.key.toLowerCase() === 'z') { e.preventDefault(); if (e.shiftKey) redo(); else undo(); }
      if (e.key === 'Delete' && selected.size) { commit(entries.filter((x) => !selected.has(x))); setSelected(new Set()); }
    };
    window.addEventListener('keydown', k);
    return () => window.removeEventListener('keydown', k);
  });

  const add = () => {
    const items = draft.split(/[\s,;]+/).map((s) => s.trim().replace(/^https?:\/\//, '').replace(/\/.*$/, '').toLowerCase()).filter(Boolean);
    const re = kind === 'ips' ? IP_RE : DOMAIN_RE;
    const good = items.filter((x) => re.test(x));
    const bad = items.length - good.length;
    const have = new Set(entries.map((x) => x.toLowerCase()));
    const fresh = [...new Set(good)].filter((x) => !have.has(x));
    if (!fresh.length) {
      toast.show({ title: 'Ничего не добавлено', body: kind === 'ips' ? 'Записи уже в списке или это не IP-адреса и не подсети (CIDR).' : 'Записи уже в списке или это не домены.' });
      return;
    }
    commit([...fresh, ...entries]);
    setDraft('');
    toast.show({ tone: 'ok', title: `Добавлено: ${count(kind, fresh.length)}`, body: bad ? `Пропущено неподходящих строк: ${bad}. Не забудьте сохранить.` : 'Не забудьте сохранить список.' });
  };

  const afterWrite = async (msg: string) => {
    await refreshInfo();
    if (core.status.zapret.state === 'running') {
      const ok = await core.restart('zapret');
      toast.show({ tone: ok ? 'ok' : 'error', title: msg, body: ok ? 'Zapret перезапущен — записи уже работают.' : 'Zapret не перезапустился — включите его заново.' });
    } else toast.show({ tone: 'ok', title: msg, body: 'Записи начнут работать при следующем запуске Zapret.' });
  };

  const save = async () => {
    setBusy(true);
    try {
      const r = await api.listWrite(file, entries);
      await loadFile(file);
      await afterWrite(`Список сохранён: ${count(kind, r.saved)}`);
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не удалось сохранить список', body: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  const switchTo = (f: string) => {
    if (f === file) return;
    if (dirty) { setNextFile(f); setConfirm('switch'); } else setFile(f);
  };

  const visible = useMemo(() => (query ? entries.filter((e) => e.includes(query.toLowerCase())) : entries), [entries, query]);
  const toggleSel = (e: string, ev: React.MouseEvent) => {
    setSelected((s) => {
      const n = new Set(ev.ctrlKey || ev.shiftKey ? s : []);
      if (s.has(e) && (ev.ctrlKey || ev.shiftKey)) n.delete(e); else n.add(e);
      return n;
    });
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Списки Zapret</h1>
          <p>Домены и адреса, к которым применяются правила стратегии, и исключения из обхода. Свои записи переносятся при обновлении сборки.</p>
        </div>
        <div className="page-head__actions">
          <button className="btn btn--ghost" onClick={() => setConfirm('factory')} disabled={locked}><Icon name="history" size={16} /> Заводские списки</button>
        </div>
      </div>

      {!admin && core.info && <Banner tone="warn" icon="shield" title="Только просмотр">Изменять списки можно с правами администратора — они лежат в защищённой папке Zapret.</Banner>}
      {core.tester.running && <Banner tone="info" icon="flask" title="Идёт тест стратегий">Изменить списки можно после его окончания.</Banner>}

      <div className="lists-layout">
        <nav className="lists-nav glass" aria-label="Файлы списков">
          <div className="nav__label">Ваши списки</div>
          {lists.filter((l) => l.user).map((l) => <ListBtn key={l.file} l={l} on={l.file === file} onClick={() => switchTo(l.file)} />)}
          <div className="nav__label" style={{ marginTop: 8 }}>Списки сборки</div>
          {lists.filter((l) => !l.user).map((l) => <ListBtn key={l.file} l={l} on={l.file === file} onClick={() => switchTo(l.file)} />)}
          {core.config.zapret.ipsetMode === 'any' && <div className="field__hint" style={{ padding: '8px 12px' }}>Режим IPSet «Все адреса»: обход по IP применяется ко всему трафику, ваши IP-адреса не нужны.</div>}
        </nav>

        <section className="glass card lists-editor">
          <div className="card__head">
            <div>
              <h2>{info?.title ?? file} <span className="card__count">{count(kind, entries.length)}{dirty ? ' · не сохранено' : ''}</span></h2>
              <p className="lead">{info?.desc} <span className="mono muted">{file}</span></p>
            </div>
            <div className="card__actions">
              {info?.hasBackup && <button className="btn btn--icon" title="Вернуть список к состоянию до первой правки" aria-label="Бэкап" onClick={() => setConfirm('backup')} disabled={locked}><Icon name="undo" size={16} /></button>}
            </div>
          </div>

          <div className="adder">
            <textarea className="textarea" rows={3} value={draft} disabled={locked} onChange={(e) => setDraft(e.target.value)}
              placeholder={kind === 'ips' ? 'IP-адреса и подсети, по одному в строке:\n149.154.160.0/20\n203.0.113.7' : 'Домены, по одному в строке:\nexample.com\n^only-this.example.com — без поддоменов'}
              onKeyDown={(e) => { if (e.key === 'Enter' && e.ctrlKey) add(); }} />
            <button className="btn" onClick={add} disabled={locked || !draft.trim()}><Icon name="plus" size={16} /> Добавить</button>
          </div>

          <div className="entries-bar">
            <div className="input-icon">
              <Icon name="search" size={15} />
              <input className="input" placeholder="Поиск" value={query} onChange={(e) => setQuery(e.target.value)} />
            </div>
            <span style={{ flex: 1 }} />
            {selected.size > 0 && <button className="btn btn--ghost btn--danger" onClick={() => { commit(entries.filter((x) => !selected.has(x))); setSelected(new Set()); }} disabled={locked}><Icon name="trash" size={15} /> Удалить ({selected.size})</button>}
            <button className="btn btn--ghost btn--icon" aria-label="Отменить" title="Отменить (Ctrl+Z)" onClick={undo} disabled={!past.length}><Icon name="undo" size={15} /></button>
          </div>

          <div className="entries" role="listbox" aria-multiselectable="true" aria-label="Записи списка">
            {visible.length === 0 && <div className="entries__empty">{entries.length ? 'Ничего не найдено' : 'Список пуст'}</div>}
            {visible.map((e) => (
              <div key={e} role="option" aria-selected={selected.has(e)} className={`entry${selected.has(e) ? ' entry--sel' : ''}`} onClick={(ev) => toggleSel(e, ev)}>
                <span className="mono">{e}</span>
                <button className="entry__del" aria-label={`Удалить ${e}`} disabled={locked} onClick={(ev) => { ev.stopPropagation(); commit(entries.filter((x) => x !== e)); }}><Icon name="close" size={13} /></button>
              </div>
            ))}
          </div>

          <div className="card__foot">
            <span className="field__hint" style={{ marginRight: 'auto', alignSelf: 'center' }}>Ctrl+клик — несколько · Delete — удалить · Ctrl+Z — отменить</span>
            <button className="btn btn--ghost" disabled={!dirty} onClick={() => { setEntries(saved); setPast([]); setFuture([]); }}>Отменить изменения</button>
            <button className="btn btn--primary" disabled={!dirty || locked || busy} onClick={save}>{busy ? <Spinner size={14} /> : <Icon name="check" size={16} />} Сохранить</button>
          </div>
        </section>
      </div>

      <Confirm open={confirm === 'factory'} onClose={() => setConfirm(null)} title="Вернуть заводские списки?" confirm="Вернуть" danger
        body="Все списки вернутся к состоянию установленной сборки Zapret, ваши записи будут удалены." onConfirm={async () => {
          try { const n = await api.listsFactoryReset(); await loadFile(file); await afterWrite(`Списки восстановлены: ${n}`); } catch (e) { toast.show({ tone: 'error', title: 'Не удалось восстановить', body: (e as Error).message }); }
        }} />
      <Confirm open={confirm === 'backup'} onClose={() => setConfirm(null)} title="Вернуть список из бэкапа?" confirm="Вернуть"
        body="Список вернётся к состоянию до вашей первой правки в этой версии сборки." onConfirm={async () => {
          try { await api.listRestoreBackup(file); await loadFile(file); await afterWrite('Список восстановлен'); } catch (e) { toast.show({ tone: 'error', title: 'Не удалось восстановить', body: (e as Error).message }); }
        }} />
      <Confirm open={confirm === 'switch'} onClose={() => setConfirm(null)} title="Изменения не сохранены" confirm="Перейти без сохранения"
        body="Несохранённые правки в этом списке будут отменены. Файл на диске не изменится." onConfirm={() => nextFile && setFile(nextFile)} />
    </div>
  );
}

function ListBtn({ l, on, onClick }: { l: ListInfo; on: boolean; onClick: () => void }) {
  return (
    <button className="nav__item lists-nav__item" aria-current={on ? 'page' : undefined} onClick={onClick}>
      <Icon name={l.kind === 'ips' ? 'globe' : 'lists'} size={16} />
      <span className="lists-nav__title">{l.title}</span>
      <span className="lists-nav__count">{l.count}</span>
    </button>
  );
}
