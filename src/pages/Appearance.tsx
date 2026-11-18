import { useMemo, useState, type CSSProperties } from 'react';
import { BrandLogo } from '../brand/BrandLogo';
import { Setting } from '../components/bits';
import { Icon } from '../components/Icon';
import { Confirm, Seg, Slider } from '../components/ui';
import { useAppearance } from '../theme/AppearanceProvider';
import { computeVars, systemMode, type Appearance as A, type TrailPalette } from '../theme/engine';
import { customTheme, THEME_BY_ID, THEMES, type ThemeDef } from '../theme/themes';

const pct = (v: number) => `${Math.round(v * 100)}%`;
const SPEEDS: Record<string, string> = { '0.5': 'медленно', '1': 'обычно', '1.5': 'быстро', '2': 'очень быстро' };

/** A real fragment of the VELIORA interface, painted with the given theme's tokens. */
function ThemePreview({ theme, a }: { theme: ThemeDef; a: A }) {
  const vars = useMemo(() => computeVars(theme, { ...a, liquidGlass: true, accent: null }), [theme, a]);
  return (
    <div className="tp" style={vars as CSSProperties} data-mode={theme.mode} aria-hidden="true">
      <div className="tp__scene">
        <span className="tp__blob tp__blob--1" />
        <span className="tp__blob tp__blob--2" />
        <span className="tp__blob tp__blob--3" />
      </div>
      <div className="tp__side">
        <span className="tp__logo" />
        <span className="tp__nav tp__nav--on" />
        <span className="tp__nav" />
        <span className="tp__nav" />
        <span className="tp__nav" />
      </div>
      <div className="tp__main">
        <span className="tp__title" />
        <div className="tp__card">
          <span className="tp__orb" />
          <span className="tp__lines"><i /><i /></span>
          <span className="tp__switch" />
        </div>
        <div className="tp__row">
          <span className="tp__btn" />
          <span className="tp__chip" />
        </div>
      </div>
    </div>
  );
}

function ThemeCard({ id, theme, title, desc, a, active, onPick, badge }: {
  id: string; theme: ThemeDef; title: string; desc: string; a: A; active: boolean; onPick: () => void; badge?: string;
}) {
  return (
    <button className={`theme-card glass${active ? ' is-active' : ''}`} onClick={onPick} aria-pressed={active} data-theme-card={id}
      onPointerMove={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        e.currentTarget.style.setProperty('--tx', String((e.clientX - r.left) / r.width - 0.5));
        e.currentTarget.style.setProperty('--ty', String((e.clientY - r.top) / r.height - 0.5));
      }}
      onPointerLeave={(e) => { e.currentTarget.style.setProperty('--tx', '0'); e.currentTarget.style.setProperty('--ty', '0'); }}>
      <ThemePreview theme={theme} a={a} />
      <span className="theme-card__meta">
        <span className="theme-card__name">{title}{badge && <span className="chip">{badge}</span>}</span>
        <span className="theme-card__desc">{desc}</span>
      </span>
      <span className="theme-card__state">
        {active ? <><Icon name="check" size={14} /> Активна</> : 'Применить'}
      </span>
    </button>
  );
}

export function Appearance() {
  const { appearance: a, update, reset } = useAppearance();
  const [confirm, setConfirm] = useState(false);
  const light = THEMES.filter((t) => t.mode === 'light');
  const dark = THEMES.filter((t) => t.mode === 'dark');
  const sysTheme = THEME_BY_ID[systemMode() === 'light' ? 'liquid-light' : 'liquid-dark'];
  const c = a.cursor;
  const setCursor = (patch: Partial<A['cursor']>) => update({ cursor: { ...a.cursor, ...patch } });

  return (
    <div className="page page--wide">
      <div className="page-head">
        <div>
          <h1>Оформление</h1>
          <p>Темы, стекло, свет и движение. Всё применяется сразу и запоминается.</p>
        </div>
        <div className="page-head__actions">
          <button className="btn btn--ghost" onClick={() => setConfirm(true)}><Icon name="refresh" size={16} /> Вернуть по умолчанию</button>
        </div>
      </div>

      <section className="theme-section">
        <div className="eyebrow theme-section__label"><Icon name="sun" size={14} /> Светлые темы</div>
        <div className="theme-grid">
          {light.map((t) => <ThemeCard key={t.id} id={t.id} theme={t} title={t.name} desc={t.description} a={a} active={a.theme === t.id} onPick={() => update({ theme: t.id })} />)}
        </div>
      </section>

      <section className="theme-section">
        <div className="eyebrow theme-section__label"><Icon name="moon" size={14} /> Тёмные темы</div>
        <div className="theme-grid">
          {dark.map((t) => <ThemeCard key={t.id} id={t.id} theme={t} title={t.name} desc={t.description} a={a} active={a.theme === t.id} onPick={() => update({ theme: t.id })} />)}
        </div>
      </section>

      <section className="theme-section">
        <div className="eyebrow theme-section__label"><Icon name="monitor" size={14} /> Дополнительные режимы</div>
        <div className="theme-grid">
          <ThemeCard id="system" theme={sysTheme} title="System" badge={systemMode() === 'light' ? 'сейчас светлая' : 'сейчас тёмная'}
            desc="Следует за светлым или тёмным режимом Windows и переключается вместе с ним." a={a} active={a.theme === 'system'} onPick={() => update({ theme: 'system' })} />
          <ThemeCard id="custom" theme={customTheme(a.custom)} title="Custom" desc="Своя тема: светлая или тёмная основа, два цвета ленты и цвет фона."
            a={a} active={a.theme === 'custom'} onPick={() => update({ theme: 'custom' })} />
        </div>
        {a.theme === 'custom' && (
          <div className="glass card custom-editor">
            <h2>Своя тема</h2>
            <div className="custom-editor__row">
              <Seg label="Основа" value={a.custom.mode} onChange={(m) => update({ custom: { ...a.custom, mode: m } })}
                options={[{ value: 'light', label: 'Светлая', icon: 'sun' }, { value: 'dark', label: 'Тёмная', icon: 'moon' }]} />
              {([['accent', 'Первая лента'], ['accent2', 'Вторая лента'], ['background', 'Фон']] as const).map(([k, label]) => (
                <label key={k} className="swatch">
                  <input type="color" value={a.custom[k]} onChange={(e) => update({ custom: { ...a.custom, [k]: e.target.value } })} />
                  <span className="swatch__chip" style={{ background: a.custom[k] }} />
                  <span>{label}</span>
                </label>
              ))}
            </div>
          </div>
        )}
      </section>

      <div className="grid grid--2 section-gap" style={{ alignItems: 'start' }}>
        <section className="glass card">
          <h2>Персонализация</h2>
          <p className="lead">Тонкая настройка поверх выбранной темы.</p>
          <div className="accent-row">
            <div>
              <div className="setting__title">Акцентный цвет</div>
              <div className="setting__desc">{a.accent ? 'Свой цвет вместо цвета темы.' : 'Используется цвет темы.'}</div>
            </div>
            <div className="accent-row__controls">
              {['#4f9bff', '#8a6cff', '#2cc0ec', '#e06cd0', '#3fbf8f', '#f0a04b'].map((col) => (
                <button key={col} className={`accent-dot${a.accent === col ? ' is-on' : ''}`} style={{ background: col }} aria-label={`Акцент ${col}`} onClick={() => update({ accent: col })} />
              ))}
              <label className="accent-dot accent-dot--pick" aria-label="Свой цвет">
                <input type="color" value={a.accent ?? '#8dbdff'} onChange={(e) => update({ accent: e.target.value })} />
                <Icon name="drop" size={14} />
              </label>
              {a.accent && <button className="btn btn--ghost btn--icon" title="Цвет темы" aria-label="Вернуть цвет темы" onClick={() => update({ accent: null })}><Icon name="close" size={14} /></button>}
            </div>
          </div>
          <div className="sliders">
            <Slider label="Размытие стекла" value={a.blur} min={0} max={2} onChange={(v) => update({ blur: v })} />
            <Slider label="Прозрачность панелей" value={a.opacity} min={0.4} max={1.6} onChange={(v) => update({ opacity: v })} format={(v) => (v < 1 ? `прозрачнее · ${pct(v)}` : v > 1 ? `плотнее · ${pct(v)}` : 'как в теме')} />
            <Slider label="Стеклянные эффекты" value={a.glass} min={0} max={2} onChange={(v) => update({ glass: v })} hint="Блики, кромки и отражения на стекле." />
            <Slider label="Яркость свечения" value={a.glow} min={0} max={2} onChange={(v) => update({ glow: v })} />
            <Slider label="Интенсивность градиентов" value={a.gradient} min={0} max={2} onChange={(v) => update({ gradient: v })} hint="Световые пятна за стеклом." />
            <Slider label="Насыщенность цветов" value={a.saturation} min={0.4} max={1.6} onChange={(v) => update({ saturation: v })} />
            <Slider label="Скругление" value={a.radius} min={0.5} max={1.6} onChange={(v) => update({ radius: v })} />
            <Slider label="Тени" value={a.shadow} min={0} max={2} onChange={(v) => update({ shadow: v })} />
            <Slider label="Скорость анимаций" value={a.speed} min={0.5} max={2} step={0.25} onChange={(v) => update({ speed: v })} format={(v) => SPEEDS[String(v)] ?? `×${v}`} />
          </div>
        </section>

        <div className="stack">
          <section className="glass card">
            <h2>Эффекты и движение</h2>
            <Setting title="Liquid Glass" desc="Многослойное стекло с размытием, бликами и глубиной. Выключите — будут плотные поверхности." checked={a.liquidGlass} onChange={(v) => update({ liquidGlass: v })} />
            <Setting title="Преломление" desc="Линза по краям круглого стекла (кнопки питания, индикатор меню)." checked={a.refraction} disabled={!a.liquidGlass} onChange={(v) => update({ refraction: v })} />
            <Setting title="Фоновые анимации" desc="Медленно плывущие световые пятна за стеклом." checked={a.bgAnimation} onChange={(v) => update({ bgAnimation: v })} />
            <Setting title="Анимации интерфейса" desc="Переходы страниц, появление карточек, микроанимации иконок и кнопок." checked={a.uiAnimation} onChange={(v) => update({ uiAnimation: v })} />
            <Setting title="Уменьшенная анимация" desc="Только короткие и необходимые переходы, без движения фона." checked={a.reducedMotion} disabled={!a.uiAnimation} onChange={(v) => update({ reducedMotion: v })} />
            <Setting title="Сохранять настройки оформления" desc={a.persist ? 'Тема и настройки запоминаются между запусками.' : 'Изменения действуют до закрытия приложения.'} checked={a.persist} onChange={(v) => update({ persist: v })} />
          </section>

          <section className="glass card">
            <h2>След за курсором</h2>
            <p className="lead">Жидкие чернила текут за стеклом вслед за указателем: панели размывают их, как настоящее стекло. Клики, выделение и прокрутка не затрагиваются.</p>
            <Setting title="Показывать след" desc="Выключенный след освобождает видеокарту." checked={c.enabled} onChange={(v) => setCursor({ enabled: v })} />
            <div className={`trail-opts${c.enabled ? '' : ' is-off'}`}>
              <Seg<TrailPalette> label="Цвета следа" value={c.palette} disabled={!c.enabled} onChange={(v) => setCursor({ palette: v })}
                options={[{ value: 'theme', label: 'Цвета темы' }, { value: 'accent', label: 'Акцент' }, { value: 'rainbow', label: 'Радуга' }]} />
              <div className="sliders">
                <Slider label="Интенсивность" value={c.intensity} min={0.1} max={2} onChange={(v) => setCursor({ intensity: v })} />
                <Slider label="Размер следа" value={c.size} min={0.3} max={2.5} onChange={(v) => setCursor({ size: v })} />
                <Slider label="Длительность (скорость исчезновения)" value={c.trail} min={0.3} max={3} onChange={(v) => setCursor({ trail: v })}
                  format={(v) => (v < 0.7 ? 'быстро гаснет' : v < 1.6 ? 'средне' : 'долго держится')} />
                <Slider label="Завихрения" value={c.swirl} min={0} max={8} step={0.5} onChange={(v) => setCursor({ swirl: v })} format={(v) => v.toFixed(1)} />
                <Slider label="Яркость свечения" value={c.glow} min={0} max={1} onChange={(v) => setCursor({ glow: v })} hint="В светлых темах чернила не светятся, а окрашивают стекло." />
              </div>
              <Setting title="Всплеск по клику" desc="Капля чернил там, где вы нажали." checked={c.splash} onChange={(v) => setCursor({ splash: v })} />
              <div className="trail-pad">
                <BrandLogo variant="symbol" size={28} decorative />
                <span>Проведите курсором по свободному месту, чтобы увидеть след</span>
              </div>
            </div>
          </section>
        </div>
      </div>

      <Confirm open={confirm} onClose={() => setConfirm(false)} title="Вернуть оформление по умолчанию?" confirm="Вернуть"
        body="Тема Liquid Glass Dark, стандартные эффекты, анимации и след курсора. Остальные настройки приложения не изменятся." onConfirm={reset} />
    </div>
  );
}
