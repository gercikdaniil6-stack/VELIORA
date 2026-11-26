import { useEffect, useMemo, useRef, useState } from 'react';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Seg } from '../components/ui';
import { api, copyText, listen, pickFile, type LogEntry } from '../lib/api';

const SOURCES: { id: 'all' | LogEntry['source']; label: string }[] = [
  { id: 'all', label: 'Все' },
  { id: 'tgws', label: 'Telegram' },
  { id: 'zapret', label: 'Zapret' },
  { id: 'test', label: 'Тест' },
  { id: 'proxy', label: 'Прокси' },
  { id: 'app', label: 'Приложение' },
];
const SRC_LABEL: Record<string, string> = { tgws: 'Telegram', zapret: 'Zapret', test: 'Тест', proxy: 'Прокси', app: 'Система' };

const time = (ms: number) => new Date(ms).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit', second: '2-digit' });

export function Logs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  useEffect(() => {
    let live = true;
    let loading = true;
    const queued: LogEntry[] = [];
    const off = listen<LogEntry>('log://entry', (entry) => {
      if (!live) return;
      if (loading) {
        if (queued.length === 3000) queued.shift();
        queued.push(entry);
      } else setLogs((previous) => [...previous.slice(-2999), entry]);
    });
    void off.then(() => live ? api.logsHistory() : []).catch(() => []).then((history) => {
      if (!live) return;
      const unique = new Map([...history, ...queued].map((entry) => [entry.id, entry]));
      loading = false;
      queued.length = 0;
      setLogs([...unique.values()].sort((a, b) => a.id - b.id).slice(-3000));
    });
    return () => { live = false; void off.then((remove) => remove()).catch(() => {}); };
  }, []);
  const toast = useToast();
  const [src, setSrc] = useState<(typeof SOURCES)[number]['id']>('all');
  const [problems, setProblems] = useState(false);
  const [q, setQ] = useState('');
  const [follow, setFollow] = useState(true);
  const box = useRef<HTMLDivElement>(null);

  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return logs.filter((l) => (src === 'all' || l.source === src) && (!problems || l.level !== 'info') && (!needle || l.text.toLowerCase().includes(needle)));
  }, [logs, src, problems, q]);
  const problemCount = useMemo(() => logs.filter((l) => l.level !== 'info').length, [logs]);

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [shown, follow]);

  const copyProblems = async () => {
    const errors = shown.filter((entry) => entry.level !== 'info');
    try {
      await copyText(errors.map((entry) => [time(entry.time), SRC_LABEL[entry.source], entry.text].join(' · ')).join('\n'));
      toast.show({ tone: 'ok', title: 'Ошибки скопированы', body: 'Перед отправкой проверьте текст на личные данные.' });
    } catch {
      toast.show({ tone: 'error', title: 'Не удалось скопировать ошибки' });
    }
  };

  const exportLogs = async () => {
    try {
      const path = await pickFile({ save: true, title: 'Сохранить отчёт для поддержки', defaultPath: `veliora-log-${new Date().toISOString().slice(0, 10)}.txt` });
      if (!path) return;
      const n = await api.logsExport(path);
      toast.show({ tone: 'ok', title: 'Сохранён отчёт для поддержки', body: `${n} записей с категориями ошибок.` });
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не удалось сохранить журнал', body: (e as Error).message });
    }
  };

  return (
    <div className="page page--fill">
      <div className="page-head">
        <div>
          <h1>Журнал</h1>
          <p>События Telegram-прокси, Zapret, теста стратегий и самого приложения. Хранится 7 дней.</p>
        </div>
        <div className="page-head__actions">
          <button className="btn" onClick={exportLogs}><Icon name="download" size={16} /> Сохранить отчёт</button>
          <button className="btn btn--ghost" disabled={!shown.some((entry) => entry.level !== 'info')} onClick={copyProblems}>Скопировать ошибки</button>
          <button className="btn btn--ghost" onClick={() => { api.logsClear().catch(() => {}); setLogs([]); }}><Icon name="trash" size={16} /> Очистить</button>
        </div>
      </div>

      <div className="logs-bar">
        <Seg label="Источник" value={src} onChange={setSrc} options={SOURCES.map((s) => ({ value: s.id, label: s.label }))} />
        <button className={`btn${problems ? ' btn--warn-on' : ''}`} aria-pressed={problems} onClick={() => setProblems(!problems)}>
          <Icon name="alert" size={15} /> Проблемы {problemCount > 0 && <span className="count-pill">{problemCount}</span>}
        </button>
        <div className="input-icon" style={{ flex: 1, minWidth: 200 }}>
          <Icon name="search" size={15} />
          <input className="input" placeholder="Поиск по тексту" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
      </div>

      <div className="logs glass" ref={box} onScroll={(e) => { const el = e.currentTarget; setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < 40); }}>
        {shown.length === 0 && <div className="entries__empty">{logs.length ? 'Ничего не найдено' : 'Пока пусто'}</div>}
        {shown.map((l) => (
          <div key={l.id} className={`log log--${l.level}`}>
            <span className="log__time mono">{time(l.time)}</span>
            <span className={`log__src log__src--${l.source}`}>{SRC_LABEL[l.source] ?? l.source}</span>
            <span className="log__text mono">{l.text}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
