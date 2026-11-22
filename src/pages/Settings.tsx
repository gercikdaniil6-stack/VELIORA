import { useState } from 'react';
import { BrandLogo } from '../brand/BrandLogo';
import type { Route } from '../components/Sidebar';
import { useAppearance } from '../theme/AppearanceProvider';
import { THEME_BY_ID } from '../theme/themes';
import { Setting } from '../components/bits';
import { Icon } from '../components/Icon';
import { useToast } from '../components/Toast';
import { Confirm, Spinner } from '../components/ui';
import { api, type AppPrefs } from '../lib/api';
import { native } from '../lib/native';
import { useCore } from '../state/core';
import { AppUpdatesSettings } from '../state/appUpdates';
import { useStore } from '../state/store';

export function Settings({ onNavigate }: { onNavigate: (r: Route) => void }) {
  const core = useCore();
  const toast = useToast();
  const proxy = useStore();
  const p = core.config.app;
  const { appearance } = useAppearance();
  const themeName = appearance.theme === 'system' ? 'System' : appearance.theme === 'custom' ? 'Своя тема' : THEME_BY_ID[appearance.theme]?.name;
  const [confirm, setConfirm] = useState<null | 'reset' | 'restart'>(null);
  const [checking, setChecking] = useState(false);

  const set = (patch: Partial<AppPrefs>) => core.save((c) => ({ ...c, app: { ...c.app, ...patch } }), { quiet: true }).then((ok) => { if (ok) core.refreshInfo(); });

  const checkNow = async () => {
    setChecking(true);
    try {
      const r = await api.checkUpdates();
      const lines: string[] = [];
      lines.push(r.zapretUpdate ? `Zapret: доступна ${r.zapretLatest} (установлена ${r.zapretInstalled}) — обновление на странице Zapret.` : `Zapret ${r.zapretInstalled} — актуальная версия.`);
      if (r.tgwsLatest) lines.push(r.tgwsUpdate ? `TgWsProxy: в исходниках вышла ${r.tgwsLatest} (встроена ${r.tgwsInstalled}) — придёт с обновлением VELIORA App.` : `TgWsProxy ${r.tgwsInstalled} — актуальная версия.`);
      if (r.errors.length) lines.push(`Не проверено: ${r.errors.join('; ')}`);
      toast.show({ tone: r.errors.length ? 'error' : 'ok', title: r.zapretUpdate || r.tgwsUpdate ? 'Есть обновления' : 'Обновлений нет', body: lines.join(' '), ms: 9000 });
    } catch (e) {
      toast.show({ tone: 'error', title: 'Не удалось проверить обновления', body: (e as Error).message });
    } finally {
      setChecking(false);
    }
  };

  const reset = async () => {
    try {
      if (proxy.conn.phase === 'connected') await proxy.disconnect();
      await api.configFactoryReset();
      toast.show({ tone: 'ok', title: 'Настройки сброшены', body: 'Ключ Telegram-прокси новый — подключите прокси в Telegram заново.' });
    } catch (e) {
      toast.show({ tone: 'error', title: 'Сброс выполнен не полностью', body: (e as Error).message });
    }
  };

  const tray = p.tray;
  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Настройки</h1>
          <p>Запуск, поведение окна и трея, обновления компонентов.</p>
        </div>
      </div>

      <div className="grid grid--2" style={{ alignItems: 'start' }}>
        <div className="stack">
          <section className="glass card">
            <h2>Запуск</h2>
            <Setting title="Запускать вместе с Windows" desc="Через Планировщик заданий — сразу с правами администратора, без запроса UAC." checked={p.launchAtLogin} onChange={(v) => set({ launchAtLogin: v })} />
            <Setting title="Тихий старт" desc={tray ? 'Приложение запускается в трее, без окна.' : 'Сначала включите иконку в трее — без неё окно не вернуть.'} checked={p.silentStart} disabled={!tray} onChange={(v) => set({ silentStart: v })} />
            <Setting title="Автозапуск локального прокси" desc="Включает локальный режим Telegram при запуске приложения." checked={core.config.tgws.autoStart}
              onChange={(v) => core.save((c) => ({ ...c, tgws: { ...c.tgws, autoStart: v } }), { quiet: true })} />
            <Setting title="Автозапуск Zapret" desc={core.config.zapret.strategy ? 'Включает обход при запуске VELIORA App.' : 'Сначала выберите стратегию на странице Zapret.'}
              checked={core.config.zapret.autoStart} disabled={!core.config.zapret.strategy}
              onChange={(v) => core.save((c) => ({ ...c, zapret: { ...c.zapret, autoStart: v } }), { quiet: true })} />
            <Setting title="Заставка при запуске" desc="Короткая анимация логотипа при открытии окна." checked={p.showSplash} onChange={(v) => set({ showSplash: v })} />
          </section>

          <AppUpdatesSettings />
          <section className="glass card">
            <h2>Обновления компонентов</h2>
            <Setting title="Проверять обновления автоматически" desc="Сообщает о новых сборках Zapret и TgWsProxy на главной." checked={p.autoCheckUpdates} onChange={(v) => set({ autoCheckUpdates: v })} />
            <div className="setting">
              <div>
                <div className="setting__title">Проверить сейчас</div>
                <div className="setting__desc">Работает и при выключенной автопроверке.</div>
              </div>
              <button className="btn" onClick={checkNow} disabled={checking}>{checking ? <Spinner size={14} /> : <Icon name="refresh" size={16} />} Проверить</button>
            </div>
          </section>
        </div>

        <div className="stack">
          <section className="glass card">
            <h2>Окно и трей</h2>
            <Setting title="Иконка в трее" desc="Значок VELIORA рядом с часами: быстрые переключатели Telegram, Zapret и прокси." checked={tray} onChange={(v) => set({ tray: v })} />
            <Setting title="Закрытие прячет в трей" desc={tray ? (p.closeToTray ? 'Кнопка «×» прячет окно, всё продолжает работать.' : 'Кнопка «×» сворачивает окно на панель задач.') : 'Без трея кнопка «×» завершает работу приложения.'}
              checked={tray && p.closeToTray} disabled={!tray} onChange={(v) => set({ closeToTray: v })} />
            <Setting title="Убрать с панели задач" desc={tray ? 'Окно открывается только из трея.' : 'Доступно при включённой иконке в трее.'} checked={tray && p.hideTaskbar} disabled={!tray} onChange={(v) => set({ hideTaskbar: v })} />
          </section>

          <section className="glass card">
            <h2>Внешний вид</h2>
            <button className="tile tile--plain" onClick={() => onNavigate('appearance')}>
              <span className="tile__icon"><BrandLogo variant="symbol" size={22} decorative /></span>
              <span className="tile__text">
                <span className="tile__title">Оформление</span>
                <span className="tile__sub">Тема: {themeName} · стекло, анимации, след курсора</span>
              </span>
              <Icon name="chevron" size={16} />
            </button>
          </section>

          <section className="glass card">
            <h2>Дополнительно</h2>
            <Setting title="Автоматически устранять конфликты Zapret"
              desc="Одна попытка восстановить Zapret после сбоя. Сторонние службы останавливаются автоматически только после сохранённого разрешения."
              checked={core.config.zapret.autoResolveConflicts ?? true}
              onChange={(value) => core.save((c) => ({ ...c, zapret: { ...c.zapret, autoResolveConflicts: value } }), { quiet: true })} />
            <div className="setting">
              <div><div className="setting__title">Разрешения Zapret</div><div className="setting__desc">Отозвать сохранённые разрешения на остановку сторонних служб.</div></div>
              <button className="btn" disabled={!core.info?.admin} onClick={() => void api.forgetZapretPermissions()
                .then(() => toast.show({ tone: 'ok', title: 'Разрешения отозваны' }))
                .catch((e: Error) => toast.show({ tone: 'error', title: 'Не удалось отозвать разрешения', body: e.message }))}>Отозвать</button>
            </div>
            <div className="home-actions">
              <button className="btn" onClick={() => onNavigate('lists')}><Icon name="lists" /> Списки Zapret</button>
              <button className="btn" onClick={() => onNavigate('logs')}><Icon name="logs" /> Журнал</button>
              <button className="btn" onClick={() => onNavigate('about')}><Icon name="about" /> О приложении</button>
            </div>
            <div className="setting">
              <div><div className="setting__title">Перезапустить приложение</div><div className="setting__desc">Telegram-прокси и Zapret остановятся; без автозапуска их нужно будет включить снова.</div></div>
              <button className="btn" onClick={() => setConfirm('restart')}>Перезапустить</button>
            </div>
            <div className="setting">
              <div><div className="setting__title">Заводские настройки</div><div className="setting__desc">Сменит ключ Telegram, сбросит стратегию, тест и настройки обхода. Списки Zapret и профили прокси сохранятся.</div></div>
              <button className="btn btn--danger" onClick={() => setConfirm('reset')}>Сбросить…</button>
            </div>
            <div className="setting">
              <div><div className="setting__title">Выйти из VELIORA App</div><div className="setting__desc">Остановит прокси и Zapret, восстановит прежние настройки Windows.</div></div>
              <button className="btn" onClick={() => native.quit()}>Выход</button>
            </div>
          </section>
        </div>
      </div>

      <Confirm open={confirm === 'reset'} onClose={() => setConfirm(null)} title="Сбросить к заводским настройкам?" confirm="Сбросить" danger
        body="Сбросятся настройки приложения, выбранная стратегия, результаты теста и настройки обхода. Будет создан новый ключ Telegram-прокси — ссылку придётся добавить в Telegram заново." onConfirm={reset} />
      <Confirm open={confirm === 'restart'} onClose={() => setConfirm(null)} title="Перезапустить VELIORA App?" confirm="Перезапустить"
        body="Приложение закроется и откроется снова." onConfirm={() => api.restart().catch((e) => toast.show({ tone: 'error', title: 'Не удалось перезапустить', body: e.message }))} />
    </div>
  );
}
