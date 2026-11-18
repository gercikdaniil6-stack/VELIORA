import { useEffect, useState } from 'react';
import { BrandLogo } from '../brand/BrandLogo';
import { isTauri, native } from '../lib/native';
import { Icon } from './Icon';

/** Custom frameless title bar: brand on the left, drag region, window controls. */
export function TitleBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let off = () => {};
    const sync = () => native.window.isMaximized().then(setMaximized);
    sync();
    native.window.onResized(sync).then((f) => (off = f));
    return () => off();
  }, []);

  return (
    <header className="titlebar">
      <div className="titlebar__brand" data-tauri-drag-region>
        <BrandLogo variant="inline" size={20} />
      </div>
      <div className="titlebar__drag" data-tauri-drag-region onDoubleClick={() => native.window.toggleMaximize()}>
        {!isTauri && <span className="preview-badge">Браузерный предпросмотр</span>}
      </div>
      <div className="titlebar__controls">
        <button className="winbtn" aria-label="Свернуть" onClick={() => native.window.minimize()}><Icon name="min" size={16} /></button>
        <button className="winbtn" aria-label={maximized ? 'Восстановить' : 'Развернуть'} onClick={() => native.window.toggleMaximize()}>
          <Icon name={maximized ? 'restore' : 'max'} size={15} />
        </button>
        <button className="winbtn winbtn--close" aria-label="Закрыть" onClick={() => native.window.close()}><Icon name="close" size={16} /></button>
      </div>
    </header>
  );
}
