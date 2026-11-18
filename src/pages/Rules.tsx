import { useEffect, useState } from 'react';
import { Setting } from '../components/bits';
import { DEFAULT_BYPASS, bypassString, useStore } from '../state/store';

export function Rules() {
  const { data, update, conn } = useStore();
  const [text, setText] = useState(data.rules.bypass.join('\n'));
  const [saved, setSaved] = useState(true);

  useEffect(() => setSaved(text.split('\n').map((s) => s.trim()).filter(Boolean).join('\n') === data.rules.bypass.join('\n')), [text, data.rules.bypass]);

  const apply = () => {
    const list = text.split(/[\n;,]+/).map((s) => s.trim()).filter(Boolean);
    update((d) => ({ ...d, rules: { ...d.rules, bypass: Array.from(new Set(list)) } }));
    setText(Array.from(new Set(list)).join('\n'));
  };

  const auto = data.automation;
  const setAuto = (k: keyof typeof auto) => (v: boolean) => update((d) => ({ ...d, automation: { ...d.automation, [k]: v } }));

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Правила</h1>
          <p>Какие адреса идут напрямую, минуя прокси, и как VELIORA ведёт себя автоматически.</p>
        </div>
      </div>

      <div className="grid grid--2" style={{ alignItems: 'start' }}>
        <section className="glass card">
          <h2>Исключения</h2>
          <p className="lead">По одному адресу или маске в строке. Поддерживаются маски вида <span className="mono">*.example.com</span> и <span className="mono">10.*</span>.</p>
          <textarea className="textarea mono" value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} aria-label="Список исключений" />
          <div style={{ display: 'flex', gap: 8, marginTop: 12, alignItems: 'center' }}>
            <button className="btn btn--primary" disabled={saved} onClick={apply}>Сохранить</button>
            <button className="btn btn--ghost" onClick={() => setText(DEFAULT_BYPASS.join('\n'))}>По умолчанию</button>
            {conn.phase === 'connected' && !saved && <span className="field__hint">Применится при следующем подключении</span>}
          </div>
          <div className="setting" style={{ marginTop: 8 }}>
            <div>
              <div className="setting__title">Локальные адреса напрямую</div>
              <div className="setting__desc">Имена без точки (принтеры, NAS) не проходят через прокси.</div>
            </div>
            <Toggle checked={data.rules.bypassLocal} onChange={(v) => update((d) => ({ ...d, rules: { ...d.rules, bypassLocal: v } }))} />
          </div>
          <div className="field__hint" style={{ marginTop: 4 }}>Итоговая строка: <span className="mono" style={{ wordBreak: 'break-all' }}>{bypassString(data) || '—'}</span></div>
        </section>

        <section className="glass card">
          <h2>Автоматизация</h2>
          <p className="lead">Действия, которые VELIORA выполняет без вашего участия.</p>
          <Setting title="Подключаться при запуске" desc="Сразу подключить активный сервер после старта приложения." checked={auto.connectOnLaunch} onChange={setAuto('connectOnLaunch')} />
          <Setting title="Переключать на лету" desc="При выборе другого сервера во время подключения — переподключиться к нему." checked={auto.reapplyOnSwitch} onChange={setAuto('reapplyOnSwitch')} />
          <Setting title="Следить за задержкой" desc="Проверять доступность серверов при запуске и каждую минуту." checked={auto.autoProbe} onChange={setAuto('autoProbe')} />
        </section>
      </div>
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return <button type="button" role="switch" aria-checked={checked} aria-label="Локальные адреса напрямую" className="switch" onClick={() => onChange(!checked)} />;
}
