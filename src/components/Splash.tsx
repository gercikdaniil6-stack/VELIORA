import { useEffect, useState } from 'react';
import { BrandLogo } from '../brand/BrandLogo';

/** Startup screen: the mark settles in, the wordmark tightens, then it dissolves. */
export function Splash({ enabled }: { enabled: boolean }) {
  const [state, setState] = useState<'in' | 'out' | 'gone'>(enabled ? 'in' : 'gone');

  useEffect(() => {
    if (state !== 'in') return;
    const t1 = setTimeout(() => setState('out'), 1250);
    const t2 = setTimeout(() => setState('gone'), 1700);
    return () => { clearTimeout(t1); clearTimeout(t2); };
  }, [state]);

  if (state === 'gone') return null;
  return (
    <div className={`splash${state === 'out' ? ' splash--out' : ''}`} aria-hidden="true">
      <div className="splash__inner">
        <BrandLogo className="splash__mark" variant="symbol" size={96} decorative />
        <BrandLogo className="splash__word" variant="wordmark" size={18} decorative />
      </div>
    </div>
  );
}
