import { useEffect, useRef, useState } from 'react';
import { Backdrop } from './components/Backdrop';
import { CursorTrail } from './components/CursorTrail';
import { Sidebar, type Route } from './components/Sidebar';
import { Splash } from './components/Splash';
import { TitleBar } from './components/TitleBar';
import { listen } from './lib/api';
import { About } from './pages/About';
import { Appearance } from './pages/Appearance';
import { Home } from './pages/Home';
import { Lists } from './pages/Lists';
import { Logs } from './pages/Logs';
import { Overview } from './pages/Overview';
import { Rules } from './pages/Rules';
import { Servers } from './pages/Servers';
import { Settings } from './pages/Settings';
import { Telegram } from './pages/Telegram';
import { Zapret } from './pages/Zapret';
import { useCore } from './state/core';
import { useGlassPointer, useReveal, useScrollVar } from './theme/motion';

const EXIT_MS = 150;

export function App() {
  const [route, setRoute] = useState<Route>('home');
  // What is on screen: lags `route` by the exit animation.
  const [shown, setShown] = useState<Route>('home');
  const [leaving, setLeaving] = useState(false);
  const { config, loaded } = useCore();
  const [splash, setSplash] = useState<boolean | null>(null);
  const main = useRef<HTMLElement>(null);

  useGlassPointer();
  useReveal(main, shown);
  useScrollVar(main, shown);

  useEffect(() => { if (loaded && splash === null) setSplash(config.app.showSplash); }, [loaded, splash, config.app.showSplash]);
  useEffect(() => {
    const off = listen<Route>('nav://go', (r) => setRoute(r));
    return () => { off.then((f) => f()); };
  }, []);

  // Page change: the old page leaves, then the new one enters with its staggered rise.
  useEffect(() => {
    if (route === shown) return;
    if (document.documentElement.dataset.motion === 'off') { setShown(route); return; }
    setLeaving(true);
    const t = setTimeout(() => {
      setShown(route);
      setLeaving(false);
      main.current?.scrollTo({ top: 0 });
    }, EXIT_MS);
    return () => clearTimeout(t);
  }, [route, shown]);

  return (
    <>
      <Backdrop />
      <CursorTrail />
      <div className="shell">
        <TitleBar />
        <Sidebar route={route} onNavigate={setRoute} />
        <main className={`main${leaving ? ' is-leaving' : ''}`} ref={main}>
          <div className="route-view" key={shown}>
            {shown === 'home' && <Home onNavigate={setRoute} />}
            {shown === 'telegram' && <Telegram />}
            {shown === 'zapret' && <Zapret />}
            {shown === 'lists' && <Lists />}
            {shown === 'proxy' && <Overview onNavigate={setRoute} />}
            {shown === 'servers' && <Servers />}
            {shown === 'rules' && <Rules />}
            {shown === 'logs' && <Logs />}
            {shown === 'appearance' && <Appearance />}
            {shown === 'settings' && <Settings onNavigate={setRoute} />}
            {shown === 'about' && <About />}
          </div>
        </main>
      </div>
      {splash !== null && <Splash enabled={splash} />}
    </>
  );
}
