import { useEffect, useState } from 'react';
import { useCore, busy } from '../state/core';
import { api } from '../lib/api';
import { Icon } from '../components/Icon';
import { Setting } from '../components/bits';
import { Banner, Modal, SelectBox, Skeleton, Spinner } from '../components/ui';
import { useToast } from '../components/Toast';
import { invoke, isTauri } from '../lib/native';
import '../styles/telegram-proxy.css';

interface Sample {
  at: number;
  result: string;
  tcp: number | null;
  elapsed: number | null;
}
interface Server extends Sample {
  id: number;
  host: string;
  port: number;
  post: number;
  date: string;
  fav: boolean;
  success: number;
  checks: number;
  oks: number;
  streak: number;
  stale: boolean;
  verified: boolean;
}
interface SourceReport {
  attemptedAt: number;
  nextAt: number;
  durationMs: number;
  direct: boolean;
  httpStatus: number | null;
  links: number;
  valid: number;
  failures: number;
  reason: string | null;
}
const sourceNames = ['Telegram', 'MTProxy list', 'SoliSpirit'];
const routes = [
  { value: '0', label: 'По настройкам системы' },
  { value: '1', label: 'Напрямую' },
];
interface Catalog {
  items: Server[];
  total: number;
  matched: number;
  available: number;
  best: Server | null;
  selected: Server | null;
  opened: Server | null;
  auto: boolean;
  interval: number;
  sync: number;
  before: number;
  busy: string;
  error: string;
  sources: (SourceReport | null)[];
  direct: boolean;
}
const filters = [
  { value: 'all', label: 'Все серверы' },
  { value: 'ok', label: 'Доступные' },
  { value: 'fav', label: 'Избранные' },
  { value: 'failed', label: 'С ошибками' },
  { value: 'archive', label: 'В архиве' },
];
const sorts = [
  { value: 'rank', label: 'По рейтингу' },
  { value: 'latency', label: 'По времени ответа' },
  { value: 'stability', label: 'По стабильности' },
  { value: 'new', label: 'По дате публикации' },
];
const intervals = [5, 15, 30, 60, 120].map((n) => ({ value: String(n), label: n + ' мин' }));
const labels: Record<string, string> = {
  ok: 'Доступен',
  new: 'Ожидает проверки',
  timeout: 'Таймаут',
  dns: 'Ошибка DNS',
  address: 'Адрес недоступен',
  tcp: 'Нет TCP-соединения',
  local: 'Локальный порт недоступен',
  protocol: 'Telegram не ответил',
};
const stamp = (sec: number) =>
  sec
    ? new Date(sec * 1000).toLocaleString('ru-RU', {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    : '—';
const ms = (n: number | null | undefined) => (n == null ? '—' : (n === 0 ? '<1' : n) + ' мс');
const fresh = (s: Server) => s.verified && s.result === 'ok' && Date.now() / 1000 - s.at <= 1800;
const status = (s: Server) =>
  s.result === 'ok' && !fresh(s) ? 'Требуется повторная проверка' : (labels[s.result] ?? 'Неизвестно');
const address = (s: Server) => (s.host.includes(':') ? '[' + s.host + ']' : s.host) + ':' + s.port;

export function TelegramProxy() {
  const toast = useToast();
  const { relay, relayPending, power } = useCore();
  const on = relay.state === 'running';
  const switching = relayPending || busy(relay);
  const showLink = () => api.telegramShowLink().catch(e => toast.show({ tone: 'error', title: 'Не удалось открыть Telegram', body: e.message }));
  const [data, setData] = useState<Catalog | null>(null);
  const [issue, setIssue] = useState('');
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('all');
  const [sort, setSort] = useState('rank');
  const [offset, setOffset] = useState(0);
  const [revision, refresh] = useState(0);
  const [pending, setPending] = useState('');
  const [detail, setDetail] = useState<Server | null>(null);
  const [history, setHistory] = useState<Sample[] | null>(null);
  useEffect(() => {
    if (!isTauri) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const read = async () => {
      try {
        const v = await invoke<Catalog>('catalog_view', { search, filter, sort, offset });
        if (active) {
          setData(v);
          setIssue('');
          if (offset > 0 && offset >= v.matched)
            setOffset(Math.max(0, Math.floor((v.matched - 1) / 24) * 24));
        }
      } catch (e) {
        if (active) setIssue(String(e));
      }
      if (active) timer = setTimeout(read, 2500);
    };
    timer = setTimeout(read, 100);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [search, filter, sort, offset, revision]);
  useEffect(() => {
    if (!detail) return;
    let active = true;
    setHistory(null);
    invoke<Sample[]>('catalog_history', { id: detail.id })
      .then((h) => {
        if (active) setHistory(h);
      })
      .catch((e) => {
        if (active) {
          setDetail(null);
          toast.show({ tone: 'error', title: 'История недоступна', body: String(e) });
        }
      });
    return () => {
      active = false;
    };
  }, [detail, toast]);
  const action = async (name: string, value = 0) => {
    if (pending) return;
    setPending(name);
    try {
      await invoke('catalog_action', { action: name, value });
      if (name === 'refresh') toast.show({ title: 'Обновление поставлено в очередь' });
      refresh((n) => n + 1);
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не удалось выполнить действие', body: String(e) });
    } finally {
      setPending('');
    }
  };
  const pick = (s: Server | null | undefined) => {
    if (s) void power(true, s.id);
  };
  const best = data?.best;
  const measured = on ? data?.opened : best;
  const selected = data?.selected ?? null;
  return (
    <div className="mt-catalog">
      {!isTauri ? (
        <Banner title="Каталог доступен в приложении Windows" icon="info" />
      ) : (
        <>
          {issue && (
            <Banner tone="error" title="Каталог недоступен">
              {issue}
            </Banner>
          )}
          {!data && !issue && <Skeleton rows={3} height={90} />}
          {data && (
            <>
              <section className="glass card mt-hero">
                <div className="mt-heading">
                  <div>
                    <h2>{on ? 'Прокси включён' : 'Прокси для Telegram'}</h2>
                    <strong className="mt-address">
                      {on ? relay.detail : best ? address(best) : 'Автоматический подбор'}
                    </strong>
                  </div>
                  <span className="mt-emblem">
                    <Icon name="telegram" size={26} />
                  </span>
                </div>
                <p className="muted">{on ? 'При первом подключении подтвердите адрес в Telegram.' : 'Нажмите «Включить прокси». VELIORA сама подберёт и проверит сервер.'}</p>
                <div className="mt-metrics">
                  <div>
                    Ответ Telegram<b>{ms(measured?.elapsed)}</b>
                  </div>
                  <div>
                    Соединение TCP<b>{ms(measured?.tcp)}</b>
                  </div>
                  <div>
                    Успешных проверок<b>{measured ? measured.oks + ' / ' + measured.checks : '—'}</b>
                  </div>
                </div>
                <div className="mt-controls">
                  <button className="btn btn--primary" disabled={switching} aria-pressed={on} onClick={() => power(!on)}>
                    {switching ? <Spinner size={15} /> : <Icon name="power" size={15} />}
                    {switching ? (relay.state === 'stopping' ? 'Выключение…' : 'Подбираем сервер…') : on ? 'Выключить прокси' : 'Включить прокси'}
                  </button>
                  {on && <button className="btn" onClick={showLink}>Открыть в Telegram</button>}
                  <button
                    className="btn"
                    disabled={!!pending || data.busy.startsWith('Подбор')}
                    onClick={() => action('scan')}
                  >
                    <Icon name="refresh" size={15} />
                    Найти лучший прокси
                  </button>
                </div>
                {data.busy && (
                  <div className="mt-progress" role="status">
                    <Spinner size={13} />
                    {data.busy}
                  </div>
                )}
              </section>
              <section className="glass card">
                <Setting
                  title="Автовыбор"
                  desc="Периодически проверять выбранный сервер и предлагать замену при сбоях. Переход подтверждается в Telegram."
                  checked={data.auto}
                  disabled={!!pending}
                  onChange={(v) => action('auto', Number(v))}
                />
                <div className="mt-current">
                  <div>
                    <span className="muted">Выбран в VELIORA</span>
                    <strong>{selected ? address(selected) : 'Сервер ещё не выбран'}</strong>
                    {selected && (
                      <small>
                        {status(selected)} · {ms(selected.elapsed)} · успешно {selected.oks} из{' '}
                        {selected.checks}
                      </small>
                    )}
                  </div>
                  <button
                    className="btn"
                    disabled={switching || !!pending || !selected || !fresh(selected)}
                    onClick={() => pick(selected)}
                  >
                    Подключить
                  </button>
                </div>
                {data.opened && (
                  <small>
                    Последний выбранный сервер — {address(data.opened)}. Статус в самом Telegram доступен в
                    его настройках.
                  </small>
                )}
              </section>
              {data.error && (
                <Banner tone="warn" title="Обновление каталога">
                  {data.error}
                </Banner>
              )}
              <section className="glass card">
                <div className="mt-heading">
                  <div>
                    <h2>Каталог серверов</h2>
                    <p className="muted text-sm">Публичные прокси обслуживаются сторонними операторами. VELIORA не может гарантировать конфиденциальность соединения.</p>
                    <small>
                      Доступно {data.available} из {data.total} · обновлено {stamp(data.sync)}
                    </small>
                  </div>
                  <button className="btn btn--ghost" disabled={!!pending} onClick={() => action('refresh')}>
                    <Icon name="refresh" size={15} />
                    Обновить
                  </button>
                </div>
                <div className="mt-tools">
                  <input
                    className="input"
                    type="search"
                    maxLength={250}
                    aria-label="Поиск сервера"
                    placeholder="Адрес или порт"
                    value={search}
                    onChange={(e) => {
                      setSearch(e.target.value);
                      setOffset(0);
                    }}
                  />
                  <SelectBox
                    label="Фильтр"
                    value={filter}
                    options={filters}
                    onChange={(v) => {
                      setFilter(v);
                      setOffset(0);
                    }}
                  />
                  <SelectBox
                    label="Сортировка"
                    value={sort}
                    options={sorts}
                    onChange={(v) => {
                      setSort(v);
                      setOffset(0);
                    }}
                  />
                </div>
                <div className="mt-items">
                  {data.items.map((s) => (
                    <article key={s.id} className="mt-item">
                      <div>
                        <strong>{address(s)}</strong>
                        <div className="mt-line">
                          <span className={fresh(s) ? 'mt-good' : ''}>{status(s)}</span>
                          <span>Ответ {ms(s.elapsed)}</span>
                          <span>Успех {s.checks ? s.oks + '/' + s.checks : '—'}</span>
                        </div>
                        <small>{s.stale ? 'Устаревшая запись · ' : ''}Последний успех · {stamp(s.success)}</small>
                      </div>
                      <div className="mt-controls">
                        <button
                          className="btn btn--icon mt-star"
                          disabled={!!pending}
                          aria-label={s.fav ? 'Убрать из избранного' : 'В избранное'}
                          aria-pressed={s.fav}
                          onClick={() => action('favorite', s.id)}
                        >
                          {s.fav ? '★' : '☆'}
                        </button>
                        <button className="btn btn--ghost" onClick={() => setDetail(s)}>
                          История
                        </button>
                        <button className="btn" disabled={switching || !!pending || !fresh(s)} onClick={() => pick(s)}>
                          Подключить
                        </button>
                      </div>
                    </article>
                  ))}
                </div>
                {!data.items.length && (
                  <p className="muted">
                    {data.total
                      ? 'По этим условиям серверов нет.'
                      : 'Ожидаем публикации с доступными ссылками.'}
                  </p>
                )}
                {data.matched > 24 && (
                  <div className="mt-pages">
                    <button
                      className="btn"
                      disabled={offset === 0}
                      onClick={() => setOffset((n) => Math.max(0, n - 24))}
                    >
                      Назад
                    </button>
                    <span>
                      {offset + 1}–{Math.min(offset + 24, data.matched)} из {data.matched}
                    </span>
                    <button
                      className="btn"
                      disabled={offset + 24 >= data.matched}
                      onClick={() => setOffset((n) => n + 24)}
                    >
                      Далее
                    </button>
                  </div>
                )}
              </section>
              <section className="glass card">
                <div className="mt-heading">
                  <div>
                    <h2>Обновление каталога</h2>
                    <small>
                      {data.before
                        ? 'Список серверов пополняется в фоне.'
                        : 'Каталог сохраняется на этом компьютере.'}
                    </small>
                  </div>
                  <SelectBox
                    label="Интервал обновления"
                    disabled={!!pending}
                    options={intervals}
                    value={String(data.interval)}
                    onChange={(v) => action('interval', Number(v))}
                  />
                </div>
                <div className="mt-controls" style={{ marginTop: 12 }}>
                  <SelectBox label="Маршрут запросов каталога" options={routes}
                    value={data.direct ? '1' : '0'} onChange={(v) => action('route', Number(v))} disabled={!!pending} />
                  <button className="btn" onClick={() => action('refresh')} disabled={!!pending}>Проверить источники</button>
                </div>
                <p className="field__hint">Маршрут применяется к запросам каталога. Пауза источника действует до следующего разрешённого запроса.</p>
                <div className="mt-items">
                  {sourceNames.map((name, i) => {
                    const source = data.sources?.[i];
                    return <article className="mt-item" key={name}>
                      <div>
                        <strong>{name}</strong>
                        {source ? <>
                          <div className="mt-line">
                            <span className={!source.reason ? 'mt-good' : ''}>{source.httpStatus == null ? 'HTTP-статус не получен' : `HTTP ${source.httpStatus}`}</span>
                            <span>{ms(source.durationMs)}</span>
                            <span>Ссылок {source.links} · принято {source.valid}</span>
                          </div>
                          {source.reason && <small>{source.reason} Ошибок подряд — {source.failures}.</small>}
                          <div className="field__hint">{source.direct ? 'Напрямую' : 'По настройкам системы'} · проверено {stamp(source.attemptedAt)}
                            {source.nextAt > Date.now() / 1000 && ` · новый запрос после ${stamp(source.nextAt)}`}</div>
                        </> : <div className="field__hint">Ожидает запроса</div>}
                      </div>
                    </article>;
                  })}
                </div>
                <p className="mt-note">
                  Публичный прокси видит ваш IP и время соединения. Подключайтесь к серверам, оператору
                  которых доверяете.
                </p>
              </section>
            </>
          )}
        </>
      )}
      <Modal
        open={!!detail}
        onClose={() => setDetail(null)}
        title="История проверок"
        lead={detail ? address(detail) : ''}
      >
        {detail?.date && (
          <small>
            Опубликовано {new Date(detail.date).toLocaleDateString('ru-RU')}
          </small>
        )}
        {history && history.length > 0 && (
          <p className="muted">
            Последние {history.length} проверок · ошибок{' '}
            {history.filter((s) => s.result !== 'ok' && s.result !== 'timeout').length} · таймаутов{' '}
            {history.filter((s) => s.result === 'timeout').length}
          </p>
        )}
        {!history ? (
          <Skeleton rows={2} />
        ) : history.length ? (
          <ul className="mt-history">
            {history.map((s, i) => (
              <li key={s.at + '-' + i}>
                <time>{stamp(s.at)}</time>
                <span>{labels[s.result]}</span>
                <span>
                  TCP {ms(s.tcp)} · Ответ {ms(s.elapsed)}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">Проверок пока нет.</p>
        )}
      </Modal>
    </div>
  );
}
