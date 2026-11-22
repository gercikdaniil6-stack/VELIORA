import { useState } from 'react';
import { useCore } from '../state/core';
import { Modal, Spinner } from './ui';

export function ZapretConflictDialog() {
  const { conflict, setConflict, resolveConflict, pending } = useCore();
  const [remember, setRemember] = useState(false);
  if (!conflict) return null;
  const running = pending.zapret;
  const token = conflict.token;
  const close = () => { if (!running) setConflict(undefined); };
  return <Modal open onClose={close} title="Обнаружен конфликт Zapret"
    lead={conflict.label}
    foot={<>
      <button className="btn" onClick={close} disabled={running}>{token ? 'Не останавливать' : 'Понятно'}</button>
      {token && <button className="btn btn--primary" disabled={running} onClick={() => void resolveConflict(token, remember)}>
        {running && <Spinner size={16} />} Остановить службу и включить Zapret
      </button>}
    </>}>
      <div className="stack">
        <p>{token
          ? 'Эта служба использует WinDivert. VELIORA попросит Windows остановить её перед запуском своего Zapret.'
          : conflict.confirmed
            ? 'Эта программа использует WinDivert. Выключите обход в ней и повторите включение в VELIORA.'
            : 'Windows не предоставила достаточно данных об этом процессе. Закройте указанную программу, если она использует обход, и повторите попытку.'}</p>
        {token && <label className="setting">
          <span>Запомнить разрешение для этой службы</span>
          <input type="checkbox" checked={remember} disabled={running} onChange={(event) => setRemember(event.target.checked)} />
        </label>}
        <details>
          <summary>Подробнее</summary>
          <p className="muted">{conflict.image} · PID {conflict.pid}{conflict.path && <> · {conflict.path}</>}</p>
          {conflict.signatureVerified != null && <p className="muted">{conflict.signatureVerified
            ? 'Подпись Windows подтверждена.' : 'Подпись Windows не подтверждена локальной проверкой.'}</p>}
        </details>
      </div>
    </Modal>;
}
