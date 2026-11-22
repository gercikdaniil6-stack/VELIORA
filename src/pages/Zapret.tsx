import { useCallback, useEffect, useMemo, useState } from 'react';
import { DiscordDiagnostics } from '../components/DiscordDiagnostics';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Banner, Confirm, Field, Help, Seg, SelectBox, ServiceCard, Skeleton, Spinner } from '../components/ui';
import { api, type Strategy, type StrategyResult, type ZapretConfig, type ZapretOverview } from '../lib/api';
import { useCore } from '../state/core';

const PORTS_RE = /^\s*\d{1,5}(\s*-\s*\d{1,5})?(\s*,\s*\d{1,5}(\s*-\s*\d{1,5})?)*\s*$/;
const validPorts = (s: string) => PORTS_RE.test(s) && s.split(',').every((p) => {
  const [a, b] = p.split('-').map((x) => +x.trim());
  return a >= 1 && a <= 65535 && (b === undefined || (b >= a && b <= 65535));
});

function resultLine(r: StrategyResult) {
  if (r.status === 'not_started') return 'winws.exe не запустился с этой стратегией — она не оценена';
  const parts: string[] = [];
  parts.push(r.blockedTotal ? `открыла ${r.opened} из ${r.blockedTotal} заблокированных` : 'ничего не было заблокировано');
  if (r.broken) parts.push(`сломала ${r.broken} работавших`);
  if (r.latencyMs) parts.push(`${r.latencyMs} мс`);
  return parts.join(' · ');
}

const PHASE: Record<string, string> = {
  baseline: 'Проверяю сеть без обхода — что заблокировано именно у вас…',
  testing: 'Проверяю стратегии на вашей сети…',
  recheck: 'Перепроверяю лидеров, чтобы исключить случайность…',
};

export function Zapret() {
  const core = useCore();
  const toast = useToast();
  const z = core.config.zapret;
  const st = core.status.zapret;
  const { tester } = core;
  const [ov, setOv] = useState<ZapretOverview | null>(null);
  const [ports, setPorts] = useState({ tcp: z.gameTcp, udp: z.gameUdp });
  const [confirmReset, setConfirmReset] = useState<null | 'test' | 'bypass' | 'discord'>(null);
  const { releases, error: relError, loading: versionsLoading } = core.versions;
  const [pick, setPick] = useState('');
  const [installing, setInstalling] = useState(false);

  const load = useCallback(() => api.zapretOverview().then(setOv).catch(() => {}), []);
  useEffect(() => { load(); }, [load, core.info?.zapretInstalled]);
  useEffect(() => setPorts({ tcp: z.gameTcp, udp: z.gameUdp }), [z.gameTcp, z.gameUdp]);

  const set = (patch: Partial<ZapretConfig>) => core.save((c) => ({ ...c, zapret: { ...c.zapret, ...patch } }), { quiet: true });
  const report = tester.report;
  const active = ov?.strategies.find((s) => s.file === z.strategy);
  const admin = core.info?.admin ?? false;

  const choose = async (s: Strategy) => {
    if (tester.running || s.file === z.strategy) return;
    const r = report?.results[s.file];
    await set({ strategy: s.file });
    if (r && r.status !== 'passed') toast.show({ title: `«${s.name}» не прошла тест в вашей сети`, body: 'Она выбрана, но может не открывать заблокированные сайты.' });
    else toast.show({ tone: 'ok', title: `Стратегия «${s.name}» выбрана` });
  };

  const runTest = async () => {
    try {
      core.setTester((t) => ({ ...t, running: true, progress: { phase: 'baseline', current: 0, total: ov?.strategies.length ?? 0 } }));
      await api.testerRun();
    } catch (e) {
      core.setTester((t) => ({ ...t, running: false, progress: null }));
      toast.show({ tone: 'error', title: 'Тест не запущен', body: (e as Error).message });
    }
  };

  const sorted = useMemo(() => {
    const list = ov?.strategies ?? [];
    if (!report) return list;
    const rank = (s: Strategy) => {
      const r = report.results[s.file];
      if (!r) return 5;
      if (report.best.includes(s.file)) return 0;
      return r.status === 'passed' ? 1 : r.status === 'failed' ? 3 : 4;
    };
    return [...list].sort((a, b) => rank(a) - rank(b) || (report.results[b.file]?.score ?? 0) - (report.results[a.file]?.score ?? 0));
  }, [ov, report]);

  const passed = report ? Object.values(report.results).filter((r) => r.status === 'passed').length : 0;
  const prog = tester.progress;
  const pct = prog && prog.total ? Math.round((prog.phase === 'baseline' ? 0 : prog.current / prog.total) * 100) : 0;

  const activeVersion = ov?.version ?? core.info?.zapretInstalled;
  const selected = releases.find((r) => r.tag === (pick || activeVersion)) ?? releases[0];
  const installable = !!selected && selected.tag !== activeVersion && (selected.installed || !!selected.sha256);
  const install = async () => {
    if (!selected || !installable) return;
    const tag = selected.tag;
    setInstalling(true);
    const id = toast.show({ tone: 'busy', title: `Устанавливаю Zapret ${tag}…`, body: 'Скачиваю архив, проверяю SHA-256 и переношу ваши списки.' });
    try {
      const r = await api.installZapret(tag);
      toast.update(id, { tone: 'ok', title: `Установлена сборка ${r.version}`, body: `Стратегий: ${r.strategies}. Результаты прошлого теста сброшены — протестируйте заново.` });
      await core.refreshInfo();
      await core.refreshTester();
      await load();
      void core.refreshVersions();
    } catch (e) {
      toast.update(id, { tone: 'error', title: 'Не удалось сменить версию', body: (e as Error).message });
    } finally {
      setInstalling(false);
    }
  };

  const clearDiscord = async () => {
    const id = toast.show({ tone: 'busy', title: 'Закрываю Discord и очищаю кеш…' });
    try {
      const r = await api.discordClearCache();
      if (!r.installed) toast.update(id, { title: 'Discord не установлен', body: 'В профиле Windows нет папок Discord.' });
      else if (r.failed.length) toast.update(id, { tone: 'error', title: 'Кеш очищен не полностью', body: `Не удалось удалить: ${r.failed.join(', ')}. Закройте Discord вручную и повторите.` });
      else toast.update(id, { tone: 'ok', title: r.removed ? 'Кеш Discord очищен' : 'Кеш Discord уже пуст', body: 'Запустите Discord заново.' });
    } catch (e) {
      toast.update(id, { tone: 'error', title: 'Не удалось очистить кеш', body: (e as Error).message });
    }
  };

  const gameApplies = active ? active.gameFilter : true;
  const fakeOpts = [{ value: '', label: 'Стандартный (из сборки)' }, ...(ov?.fakes ?? []).map((f) => ({ value: f.file, label: f.label }))];
  const portsValid = validPorts(ports.tcp) && validPorts(ports.udp);
  const portsDirty = ports.tcp !== z.gameTcp || ports.udp !== z.gameUdp;

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Zapret</h1>
          <p>Обход DPI на уровне сетевых пакетов: открывает YouTube, Discord, игры и другие сервисы без VPN. Публичный IP не меняется.</p>
        </div>
      </div>

      <div className="stack">
        {!admin && core.info && (
          <Banner tone="warn" icon="shield" title="Нужны права администратора"
            actions={<button className="btn btn--primary" onClick={() => api.elevate().catch((e) => toast.show({ tone: 'error', title: 'Права не получены', body: e.message }))}>Запросить права</button>}>
            Без них драйвер WinDivert не загрузится: Zapret, тест стратегий и изменение списков недоступны.
          </Banner>
        )}

        <ServiceCard icon="shield" title="Обход DPI (Zapret)" status={st} pending={core.pending.zapret} version={ov?.version}
          subtitle={active ? active.name : 'Выберите стратегию ниже'}
          onToggle={(on) => { if (on && !z.strategy) { toast.show({ title: 'Сначала выберите стратегию' }); return; } core.toggle('zapret', on); }}
          disabled={tester.running || !admin} disabledReason={tester.running ? 'Идёт тест стратегий — переключатель временно недоступен' : !admin ? 'Нужны права администратора' : undefined}>
          {!z.strategy && <span className="service__note">Нужна стратегия</span>}
        </ServiceCard>

        <section className="glass card manager-version-card">
          <h2>Версия сборки</h2>
          <p className="lead">Установлена <b>{ov?.version ?? '…'}</b>{ov?.source === 'update' ? ' (обновлена из приложения)' : ' (встроенная)'}. Источник — официальные релизы Flowseal/zapret-discord-youtube, установка только с проверкой SHA-256.</p>
          <div className="manager-controls manager-version">
            {releases.length > 0 && <SelectBox label="Версия Zapret" value={selected?.tag ?? ''} onChange={setPick} disabled={installing}
              options={releases.map((r) => ({
                value: r.tag,
                label: r.tag + (r.tag === activeVersion ? ' · активная' : r.installed ? ' · доступна локально' : ''),
                detail: (r.publishedAt ? new Date(r.publishedAt).toLocaleDateString('ru-RU', { year: 'numeric', month: 'short', day: 'numeric' }) : '')
                  + (!r.sha256 && !r.installed ? ' · SHA-256 отсутствует' : ''),
                fresh: r.latest && r.tag !== activeVersion,
              }))} />}
            <button className="btn" onClick={() => void core.refreshVersions()} disabled={versionsLoading || installing}>
              {versionsLoading ? <Spinner size={16} /> : <Icon name="refresh" size={16} />} Обновить список
            </button>
            {releases.length > 0 && <button className="btn btn--primary" disabled={installing || !admin || tester.running || !installable} onClick={() => void install()}>
              {installing ? <Spinner size={14} /> : <Icon name="download" size={16} />} {selected?.installed ? 'Переключить' : 'Установить'}
            </button>}
          </div>
          {relError && <p className="field__error" role="status">{relError}</p>}
          {selected && <p className="field__hint">При смене версии VELIORA проверяет совместимость выбранной стратегии и переносит пользовательские списки.</p>}
        </section>

        <DiscordDiagnostics />

        <section className="glass card">
          <div className="card__head">
            <div>
              <h2>Стратегии {report && !tester.running && <span className="card__count">{passed} из {Object.keys(report.results).length} прошли HTTPS-проверку</span>}</h2>
              <p className="lead">HTTPS-тест сравнивает доступность сервисов без обхода и с каждой стратегией, учитывает скорость и то, что стратегия ломает, и перепроверяет лучших.</p>
            </div>
            <div className="card__actions">
              {tester.running ? (
                <button className="btn" onClick={() => api.testerCancel()}><Icon name="close" size={15} /> Отменить</button>
              ) : (
                <>
                  {report && <button className="btn btn--ghost" onClick={() => setConfirmReset('test')}>Сбросить результаты</button>}
                  <button className="btn btn--primary" onClick={runTest} disabled={!admin || !ov?.strategies.length}><Icon name="flask" size={16} /> {report ? 'Перетестировать' : 'Протестировать'}</button>
                </>
              )}
            </div>
          </div>

          {tester.running && (
            <div className="test-progress">
              <div className="test-progress__row">
                <Spinner size={16} />
                <span>{PHASE[prog?.phase ?? 'baseline'] ?? 'Тестирование…'}</span>
                <span style={{ flex: 1 }} />
                {prog && prog.phase !== 'baseline' && <span className="mono muted">{prog.current}/{prog.total}{prog.strategy ? ` · ${prog.strategy}` : ''}</span>}
              </div>
              <div className="bar"><span style={{ width: `${pct}%` }} /></div>
              <div className="field__hint">Для точного результата выключите VPN и другие программы обхода. Займёт 2–4 минуты; по окончании придёт уведомление.</div>
            </div>
          )}

          {report && !tester.running && (
            <div className="test-notes">
              {report.vpn && <Banner tone="warn" title={`Тест шёл через VPN-туннель «${report.vpn}»`}>Трафик уходил в туннель, поэтому результаты описывают VPN, а не вашу сеть. Выключите туннель и перетестируйте.</Banner>}
              {report.systemProxy && <div className="note"><Icon name="info" size={15} /> В Windows включён системный прокси — тест измерял прямое подключение.</div>}
              <div className="note"><Icon name="info" size={15} />
                {report.blocked.length ? <>Без обхода не открывались: <b>{report.blocked.join(', ')}</b>.</> : 'Без обхода открывалось всё — стратегии ранжированы по скорости и по тому, что они не ломают.'}
              </div>
              {report.unfixable.length > 0 && <div className="note note--warn"><Icon name="alert" size={15} /> Ни одна стратегия не открыла: <b>{report.unfixable.join(', ')}</b> — вероятно, блокировка по IP.</div>}
              {report.dnsIssues.length > 0 && <div className="note note--warn"><Icon name="alert" size={15} /> Проблема DNS или подменённый сертификат: <b>{report.dnsIssues.join(', ')}</b>. Поможет защищённый DNS (DoH) в браузере или Windows.</div>}
              {report.cancelled && <div className="note"><Icon name="info" size={15} /> Тест был остановлен: проверено {report.tested} из {report.total}.</div>}
            </div>
          )}

          <div className="strategies" role="radiogroup" aria-label="Стратегии">
            {!ov && <Skeleton rows={6} />}
            {ov && ov.strategies.length === 0 && <div className="muted">Сборка Zapret не найдена. Переустановите VELIORA App или установите версию выше.</div>}
            {sorted.map((s) => {
              const r = report?.results[s.file];
              const best = report?.best.includes(s.file);
              const tag = !r ? null : best ? (report!.best.length > 1 ? 'наравне' : 'лучшая') : r.status === 'passed' ? 'работает' : r.status === 'failed' ? 'не прошла' : 'не запустилась';
              const tone = !r ? '' : best ? 'best' : r.status === 'passed' ? 'ok' : 'bad';
              const selected = z.strategy === s.file;
              return (
                <button key={s.file} role="radio" aria-checked={selected} className={`strategy${selected ? ' strategy--on' : ''}${r && r.status !== 'passed' ? ' strategy--dim' : ''}`}
                  onClick={() => choose(s)} disabled={tester.running}>
                  <span className="strategy__radio" aria-hidden="true" />
                  <span className="strategy__text">
                    <span className="strategy__name">{s.name}{s.note && <span className="strategy__note">{s.note}</span>}</span>
                    {r && <span className="strategy__result">{resultLine(r)}{r.rechecked ? ' · перепроверена' : ''}</span>}
                  </span>
                  {r && r.status !== 'not_started' && (
                    <span className="strategy__services">{Object.entries(r.services).map(([svc, [ok, total]]) => (
                      <span key={svc} className={`svc ${ok === total ? 'svc--ok' : ok ? 'svc--part' : 'svc--bad'}`} title={`${svc}: открыто ${ok} из ${total}`}>{svc}</span>
                    ))}</span>
                  )}
                  {tag && <span className={`chip chip--${tone}`}>{tag}</span>}
                </button>
              );
            })}
          </div>
        </section>

        <div className="grid grid--2" style={{ alignItems: 'start' }}>
          <section className="glass card">
            <h2>Настройка обхода</h2>
            <p className="lead">Изменения применяются сразу: работающий Zapret перезапустится.</p>
            <div className="opt">
              <div className="opt__head"><Icon name="gamepad" size={16} /> Игровой фильтр <Help text="Расширяет обход на порты 1024–65535: лаунчеры, вход в игру, игровые серверы и голос. Выключите, если с играми проблем нет." /></div>
              <Seg label="Игровой фильтр" value={z.gameFilter} disabled={tester.running}
                options={[{ value: 'off', label: 'Выкл' }, { value: 'all', label: 'TCP и UDP' }, { value: 'tcp', label: 'TCP' }, { value: 'udp', label: 'UDP' }]}
                onChange={(v) => set({ gameFilter: v })} />
              <div className="field__hint">{{ off: 'Обход только для сайтов и Discord — игры не затрагиваются.', all: 'Игры, лаунчеры и голос: всё на портах из списков ниже.', tcp: 'Лаунчеры, загрузки, вход в аккаунт и чат.', udp: 'Голос, звонки и сам игровой процесс.' }[z.gameFilter]}</div>
              {!gameApplies && z.gameFilter !== 'off' && <div className="note note--warn"><Icon name="alert" size={15} /> Выбранная стратегия не использует игровой фильтр — с ней настройка не действует.</div>}
              {z.gameFilter !== 'off' && (
                <div className="form-grid form-grid--ports">
                  <Field label="Порты TCP" error={validPorts(ports.tcp) ? null : 'Порты через запятую или диапазоны «от-до»'}>
                    <input className="input mono" value={ports.tcp} disabled={z.gameFilter === 'udp'} onChange={(e) => setPorts({ ...ports, tcp: e.target.value })} />
                  </Field>
                  <Field label="Порты UDP" error={validPorts(ports.udp) ? null : 'Порты через запятую или диапазоны «от-до»'}>
                    <input className="input mono" value={ports.udp} disabled={z.gameFilter === 'tcp'} onChange={(e) => setPorts({ ...ports, udp: e.target.value })} />
                  </Field>
                  {portsDirty && <button className="btn btn--primary" disabled={!portsValid} onClick={() => set({ gameTcp: ports.tcp.replace(/\s/g, ''), gameUdp: ports.udp.replace(/\s/g, '') })}>Применить порты</button>}
                </div>
              )}
            </div>
            <div className="opt">
              <div className="opt__head"><Icon name="layers" size={16} /> Фейк для голоса Discord <Help text="Пакет-обманка, который стратегия отправляет перед голосовым трафиком Discord. Попробуйте другой, если голос не подключается." /></div>
              <SelectBox label="Фейк для Discord" value={z.fakeDiscord ?? ''} options={fakeOpts} disabled={tester.running} onChange={(v) => set({ fakeDiscord: v || null })} />
              {active && !active.fakeDiscord && z.fakeDiscord && <div className="field__hint">Выбранная стратегия не использует сменный фейк для Discord.</div>}
            </div>
            <div className="opt">
              <div className="opt__head"><Icon name="gamepad" size={16} /> Фейк для игр <Help text="Пакет-обманка для игрового UDP-трафика. Действует при включённом игровом фильтре (TCP и UDP или UDP)." /></div>
              <SelectBox label="Фейк для игр" value={z.fakeGame ?? ''} options={fakeOpts} disabled={tester.running} onChange={(v) => set({ fakeGame: v || null })} />
              {(z.gameFilter === 'off' || z.gameFilter === 'tcp') && z.fakeGame && <div className="field__hint">Сейчас не применяется: игровой UDP-фильтр выключен.</div>}
            </div>
            <div className="card__foot">
              <button className="btn btn--ghost" onClick={() => setConfirmReset('bypass')} disabled={tester.running}>Сбросить к заводским</button>
            </div>
          </section>

          <div className="stack">
            <section className="glass card">
              <h2>Режим IPSet</h2>
              <p className="lead">Какие адреса обходятся по IP — в дополнение к правилам по доменам.</p>
              <div className="ipset">
                {([
                  ['basic', 'Базовый', 'Сети Telegram и ваши IP. Минимум влияния на остальные соединения.'],
                  ['full', `Полный${ov?.ipsetFull ? ` · ${ov.ipsetFull.toLocaleString('ru-RU')}` : ''}`, 'Полный список адресов Flowseal: Discord, YouTube и другие сервисы по IP.'],
                  ['any', 'Все адреса', 'Обход по IP для всего трафика на портах стратегии — когда нужного сервиса нет в списках.'],
                ] as const).map(([v, label, desc]) => (
                  <button key={v} className={`ipset__opt${z.ipsetMode === v ? ' ipset__opt--on' : ''}`} aria-pressed={z.ipsetMode === v} disabled={tester.running} onClick={() => set({ ipsetMode: v })}>
                    <span className="ipset__label">{label}</span>
                    <span className="ipset__desc">{desc}</span>
                  </button>
                ))}
              </div>
            </section>

            <section className="glass card">
              <h2>Discord</h2>
              <p className="lead">Если после смены стратегии Discord всё ещё не подключается — очистите его кеш.</p>
              <button className="btn" onClick={() => setConfirmReset('discord')}><Icon name="trash" size={16} /> Очистить кеш Discord</button>
            </section>


          </div>
        </div>
      </div>

      <Confirm open={confirmReset === 'test'} onClose={() => setConfirmReset(null)} title="Сбросить результаты теста?" confirm="Сбросить" danger
        body="Отметки и рейтинг стратегий будут удалены. Выбранная стратегия останется." onConfirm={() => api.testerReset().then(core.refreshTester).catch((e) => toast.show({ tone: 'error', title: 'Не удалось', body: e.message }))} />
      <Confirm open={confirmReset === 'bypass'} onClose={() => setConfirmReset(null)} title="Сбросить настройки обхода?" confirm="Сбросить"
        body="Игровой фильтр выключится (порты вернутся к 1024–65535), фейки и режим IPSet — к стандартным. Списки и стратегия не изменятся."
        onConfirm={() => set({ gameFilter: 'off', gameTcp: '1024-65535', gameUdp: '1024-65535', fakeDiscord: null, fakeGame: null, ipsetMode: 'basic' })} />
      <Confirm open={confirmReset === 'discord'} onClose={() => setConfirmReset(null)} title="Discord закроется — очистить кеш?" confirm="Очистить"
        body="VELIORA закроет Discord и удалит папки Cache, Code Cache и GPUCache. Сообщения и настройки не затрагиваются." onConfirm={clearDiscord} />
    </div>
  );
}
