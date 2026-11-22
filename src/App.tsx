import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { Backdrop } from './components/Backdrop';
import { CursorTrail } from './components/CursorTrail';
import { Sidebar, type Route } from './components/Sidebar';
import { Splash } from './components/Splash';
import { TitleBar } from './components/TitleBar';
import { listen } from './lib/api';
import { Home } from './pages/Home';
import { useCore } from './state/core';
import { AppUpdateProvider } from './state/appUpdates';
import { useGlassPointer, useReveal, useScrollVar } from './theme/motion';

const ZapretConflictDialog = lazy(() => import('./components/ZapretConflictDialog').then(m => ({ default: m.ZapretConflictDialog })));
const About = lazy(() => import('./pages/About').then(m => ({ default: m.About })));
const Appearance = lazy(() => import('./pages/Appearance').then(m => ({ default: m.Appearance })));
const Lists = lazy(() => import('./pages/Lists').then(m => ({ default: m.Lists })));
const Logs = lazy(() => import('./pages/Logs').then(m => ({ default: m.Logs })));
const Settings = lazy(() => import('./pages/Settings').then(m => ({ default: m.Settings })));
const Telegram = lazy(() => import('./pages/Telegram').then(m => ({ default: m.Telegram })));
const Zapret = lazy(() => import('./pages/Zapret').then(m => ({ default: m.Zapret })));
const BypassCatalog = lazy(() => import('./pages/ManagedSites').then(m => ({ default: m.BypassCatalog })));

const EXIT_MS = 150;

export function App() {
  const [addSite, setAddSite] = useState(false);
  const [route, setRoute] = useState<Route>('home');
  // What is on screen: lags `route` by the exit animation.
  const [shown, setShown] = useState<Route>('home');
  const [leaving, setLeaving] = useState(false);
  const { config, loaded, conflict } = useCore();
  const [splash, setSplash] = useState<boolean | null>(null);
  const main = useRef<HTMLElement>(null);

  useGlassPointer(shown);
  useReveal(main, shown);
  useScrollVar(main, shown);

  useEffect(() => { if (loaded && splash === null) setSplash(config.app.showSplash); }, [loaded, splash, config.app.showSplash]);
  useEffect(() => {
    const off = listen<Route>('nav://go', (r) => setRoute(r));
    return () => { off.then((f) => f()); };
  }, []);

  // Page change: the old page leaves, then the new one enters with its staggered rise.
  useEffect(() => {
    if (route === shown) { setLeaving(false); return; }
    if (document.documentElement.dataset.motion === 'off') { setShown(route); setLeaving(false); main.current?.scrollTo({ top: 0 }); return; }
    setLeaving(true);
    const t = setTimeout(() => {
      setShown(route);
      setLeaving(false);
      main.current?.scrollTo({ top: 0 });
    }, EXIT_MS);
    return () => clearTimeout(t);
  }, [route, shown]);

  return (
    <AppUpdateProvider>
      <Backdrop />
      <CursorTrail />
      <div className="shell">
        <TitleBar />
        <Sidebar route={route} onNavigate={setRoute} />
        <main className={`main${leaving ? ' is-leaving' : ''}`} ref={main}>
          <div className="route-view" key={shown}>
            <Suspense fallback={<div className="page muted" role="status">Открываем раздел…</div>}>
            {shown === 'home' && <Home onNavigate={setRoute} onAddSite={() => { setAddSite(true); setRoute('catalog'); }} />}
            {shown === 'telegram' && <Telegram />}
            {shown === 'zapret' && <Zapret />}
            {shown === 'lists' && <Lists />}
            {shown === 'catalog' && <BypassCatalog onNavigate={setRoute} addOnOpen={addSite} onAddOpened={() => setAddSite(false)} />}
            {shown === 'logs' && <Logs />}
            {shown === 'appearance' && <Appearance />}
            {shown === 'settings' && <Settings onNavigate={setRoute} />}
            {shown === 'about' && <About />}
            </Suspense>
          </div>
        </main>
      </div>
      {conflict && <Suspense fallback={null}><ZapretConflictDialog key={conflict.pid} /></Suspense>}
      {splash !== null && <Splash enabled={splash} />}
    </AppUpdateProvider>
  );
}
