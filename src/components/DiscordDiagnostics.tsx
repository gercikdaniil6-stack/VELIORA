import { useEffect, useState } from 'react';
import { api, type DiscordDiagnostics as Results, type DiscordUdp } from '../lib/api';
import { useCore } from '../state/core';
import { Banner, SelectBox, Spinner } from './ui';
import { Icon } from './Icon';
import { useToast } from './Toast';

const modes: Record<DiscordUdp, { label: string; help: string }> = {
  bundle: { label: 'Из стратегии', help: 'Параметры Discord UDP из выбранной сборки Zapret.' },
  discovery: { label: 'Только обнаружение', help: 'Обрабатывает начальные пакеты обнаружения. Добавляет точечное правило для высоких портов.' },
  direct: { label: 'Прямой UDP', help: 'Пропускает пакеты на стандартных диапазонах Discord через Zapret без изменения.' },
};
const tests = [
  'Собеседник слышал меня, и я слышал его.',
  'У собеседника появилось изображение моего экрана.',
  'Изображение двигалось без зависания в течение 30 секунд.',
];

export function DiscordDiagnostics() {
  const core = useCore();
  const toast = useToast();
  const cfg = core.config.zapret;
  const mode = cfg.discordUdp ?? 'bundle';
  const status = core.status.zapret;
  const saved = cfg.discordVerified;
  const busy = core.pending.zapret || core.tester.running || status.state === 'starting' || status.state === 'stopping';
  const [checks, setChecks] = useState([false, false, false]);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<Results | null>(null);
  useEffect(() => { setChecks([false, false, false]); setReport(null); }, [status.startedAt, status.state, cfg.strategy, mode]);

  const apply = async (profile: DiscordUdp, strategy = cfg.strategy) => {
    setSaving(true);
    try {
      await core.save(c => ({ ...c, zapret: { ...c.zapret, strategy, discordUdp: profile } }));
    } finally {
      setSaving(false);
    }
  };
  const diagnose = async () => {
    setRunning(true);
    try {
      setReport(await api.discordDiagnostics());
    } catch (e) {
      toast.show({ tone: 'error', title: 'Диагностика не завершилась', body: (e as Error).message });
    } finally {
      setRunning(false);
    }
  };
  const confirm = async () => {
    if (!status.startedAt) return;
    setSaving(true);
    try {
      await api.discordConfirmMedia(status.startedAt);
      toast.show({ tone: 'ok', title: 'Подтверждённый режим сохранён' });
    } catch (e) {
      toast.show({ tone: 'error', title: 'Проверка не сохранена', body: (e as Error).message });
    } finally {
      setSaving(false);
    }
  };

  return <section className="glass card">
    <div className="card__head">
      <div><h2>Discord · голос и демонстрация</h2><p className="lead">Профиль UDP накладывается на выбранную стратегию Zapret.</p></div>
      <button className="btn" disabled={running || busy} onClick={diagnose}>
        {running ? <Spinner size={16} /> : <Icon name="refresh" size={16} />} Проверить сеть
      </button>
    </div>
    <div className="stack" style={{ gap: 14 }}>
      <div>
        <SelectBox label="UDP для Discord" value={mode} disabled={busy || running || saving || !cfg.strategy}
          options={Object.entries(modes).map(([value, info]) => ({ value, label: info.label }))}
          onChange={v => apply(v as DiscordUdp)} />
        <p className="field__hint">{modes[mode].help}</p>
      </div>
      {report && <div className="stack" style={{ gap: 8 }} aria-live="polite">
        {report.competing > 0 && <Banner tone="warn" title="Работает другой экземпляр Zapret">
          Найдено сторонних процессов winws или winws2 — {report.competing}. Остановите стороннюю службу её штатным способом перед сравнительным тестом.
        </Banner>}
        <div className="muted">{report.web.map(p => <div key={p.url}>HTTPS · {p.service} — {p.ok ? `ответ за ${p.ms} мс` : ({dns:'ошибка DNS',cert:'ошибка сертификата',timeout:'таймаут',reset:'сбой соединения'}[p.fault] ?? p.fault)}</div>)}</div>
        <div>Тестовый STUN по UDP — {report.udp.received} из {report.udp.sent} ответов{report.udp.averageMs != null && ` · ${report.udp.averageMs} мс`}</div>
        {report.udp.error && <div className="field__hint">{report.udp.error}</div>}
        <div className="field__hint">Zapret — {report.status.state === 'running' ? 'запущен' : 'не запущен'}. Перехват VELIORA — {report.captureConfirmed ? 'подтверждён' : 'нет подтверждения'}. MTU активных адаптеров — {report.mtu.join(', ') || 'нет данных'}. {report.vpn ? 'Обнаружен активный адаптер VPN.' : 'Активный адаптер VPN не обнаружен.'} STUN проверяет обмен UDP с тестовым сервером Cloudflare.</div>
      </div>}
      <div>
        <h3>Ручной тест с участником канала</h3>
        <p className="field__hint">Запустите Zapret, переподключитесь к голосовому каналу и включите демонстрацию. Попросите другого участника открыть трансляцию и подтвердить результат.</p>
        {tests.map((label, index) => <label key={label} style={{ display: 'block', marginBlock: 8 }}>
          <input type="checkbox" checked={checks[index]} disabled={status.state !== 'running' || busy || saving}
            onChange={e => setChecks(old => old.map((item, i) => i === index ? e.target.checked : item))} /> {label}
        </label>)}
        <button className="btn btn--primary" disabled={!checks.every(Boolean) || busy || saving || status.state !== 'running'}
          onClick={confirm}>Сохранить подтверждённый режим</button>
      </div>
      {saved && <div>
        <p className="field__hint">Последняя ручная проверка · {saved.strategy} · {modes[saved.udp].label} · Zapret {saved.version} · {new Date(saved.at).toLocaleString()}</p>
        <button className="btn" disabled={busy || saving || (saved.strategy === cfg.strategy && saved.udp === mode)}
          onClick={() => apply(saved.udp, saved.strategy)}>Вернуть подтверждённый режим</button>
      </div>}
    </div>
  </section>;
}
