import { useEffect, useState } from 'react';
import { api, copyText, type DiagReport } from '../lib/api';
import { Icon } from './Icon';
import { useToast } from './Toast';
import { Modal, Skeleton, Spinner } from './ui';

/** "Why might it not work?" — runs the system checks and explains each finding. */
export function Diagnostics({ open, onClose }: { open: boolean; onClose: () => void }) {
  const toast = useToast();
  const [report, setReport] = useState<DiagReport | null>(null);
  const [running, setRunning] = useState(false);
  const [confirmClose, setConfirmClose] = useState(false);
  const [closing, setClosing] = useState(false);
  const [closeResult, setCloseResult] = useState('');
  const [index, setIndex] = useState(0);

  const run = async () => {
    setConfirmClose(false);
    setRunning(true);
    try {
      const r = await api.diagRun();
      setReport(r);
      setIndex(0);
    } catch (e) {
      toast.show({ tone: 'error', title: 'Проверка не выполнена', body: (e as Error).message });
    } finally {
      setRunning(false);
    }
  };

  useEffect(() => {
    if (open) run();
    else setReport(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const close = async () => {
    if (!report?.closeOffer || closing || running) return;
    setClosing(true);
    setConfirmClose(false);
    try {
      const released = await api.diagCloseListener(report.closeOffer.token);
      setCloseResult(released ? 'Порт освободился. Теперь можно запустить Telegram-прокси.'
        : 'Запрос на закрытие отправлен, но порт ещё занят. Проверьте окно программы или закройте её вручную.');
    } catch (error) {
      setCloseResult((error as Error).message);
    } finally {
      await run();
      setClosing(false);
    }
  };

  const issue = report?.issues[index];
  const act = async (kind: string) => {
    try {
      if (kind === 'elevate') await api.elevate();
      else await api.diagOpen(kind);
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не получилось', body: (e as Error).message });
    }
  };

  return (
    <Modal open={open} onClose={onClose} wide title="Почему может не работать?"
      lead="VELIORA проверяет систему и объясняет каждую найденную проблему с пошаговой инструкцией."
      foot={<>
        <button className="btn btn--ghost" onClick={onClose}>Закрыть</button>
        <button className="btn" onClick={() => { setCloseResult(''); void run(); }} disabled={running || closing}>{running ? <Spinner size={14} /> : <Icon name="refresh" size={16} />} Проверить снова</button>
      </>}>
      {running && !report && <><div className="diag-wait"><Spinner size={18} /> Проверяю систему…</div><Skeleton rows={3} height={38} /></>}
      {report && report.issues.length === 0 && (
        <div className="diag-ok">
          <span className="diag-ok__icon"><Icon name="check" size={22} /></span>
          <div>
            <div className="diag-ok__title">Системных предупреждений нет</div>
            <div className="muted">Проверено: {report.passed.join(', ')}.</div>
          </div>
        </div>
      )}
      {report && report.checks.length > 0 && (
        <section aria-label="Проверки подключения" className="diag">
          <h3 className="diag__title">Проверки подключения</h3>
          {closeResult && <div className="note" role="status">{closeResult}</div>}
          {report.checks.map((check) => (
            <div key={check.name} className="note">
              <div><strong>{check.name} · {check.status}</strong><div className="muted">{check.detail}</div></div>
            </div>
          ))}
          {report.closeOffer && (confirmClose ? (
            <div className="note">
              <div>
                <strong>Закрыть {report.closeOffer.label}?</strong>
                <div className="muted">Сначала сохраните работу в этой программе.</div>
                <button className="btn btn--ghost" onClick={() => setConfirmClose(false)}>Отмена</button>
                <button className="btn btn--primary" onClick={() => void close()}>Подтвердить закрытие</button>
              </div>
            </div>
          ) : <button className="btn" disabled={running || closing} onClick={() => setConfirmClose(true)}>Закрыть программу и проверить снова</button>)}
        </section>
      )}
      {issue && (
        <div className="diag">
          <div className="diag__nav">
            <span className={`chip ${issue.severity === 'error' ? 'chip--bad' : 'chip--warn'}`}>{issue.severity === 'error' ? 'Проблема' : 'Предупреждение'}</span>
            <span className="muted">{index + 1} из {report!.issues.length}</span>
            <span style={{ flex: 1 }} />
            <button className="btn btn--ghost btn--icon" aria-label="Предыдущая" disabled={index === 0} onClick={() => setIndex(index - 1)}><Icon name="chevron" size={16} /></button>
            <button className="btn btn--ghost btn--icon" aria-label="Следующая" disabled={index >= report!.issues.length - 1} onClick={() => setIndex(index + 1)}><Icon name="chevron" size={16} /></button>
          </div>
          <h3 className="diag__title">{issue.title}</h3>
          <div className="diag__impact">{issue.impact}</div>
          <p className="diag__detail">{issue.detail}</p>
          <div className="eyebrow">Что сделать</div>
          <ol className="diag__steps">{issue.steps.map((s, i) => <li key={i}>{s}</li>)}</ol>
          {issue.commands.length > 0 && (
            <div className="diag__cmds">
              {issue.commands.map((c) => (
                <div key={c} className="cmd">
                  <code className="mono">{c}</code>
                  <button className="btn btn--ghost btn--icon" aria-label="Копировать команду" onClick={() => { copyText(c); toast.show({ tone: 'ok', title: 'Команда скопирована' }); }}><Icon name="copy" size={15} /></button>
                </div>
              ))}
            </div>
          )}
          {issue.actions.length > 0 && (
            <div className="diag__actions">{issue.actions.map((a) => <button key={a.kind} className="btn" onClick={() => act(a.kind)}><Icon name="external" size={15} /> {a.label}</button>)}</div>
          )}
          {report!.passed.length > 0 && <div className="field__hint" style={{ marginTop: 16 }}>В порядке: {report!.passed.join(', ')}.</div>}
        </div>
      )}
    </Modal>
  );
}
