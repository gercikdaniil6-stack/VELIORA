// Brand registry — the ONLY place the app references logo files.
// Replace a file in /assets/brand (or rerun `npm run brand`) and every screen,
// the splash, the title bar and About update automatically.
import manifest from '../../assets/brand/manifest.json';

import symbolLight from '../../assets/brand/symbol.svg';
import symbolDark from '../../assets/brand/symbol-dark.svg';
import symbolMonoWhite from '../../assets/brand/symbol-monochrome-white.svg';
import symbolMonoBlack from '../../assets/brand/symbol-monochrome-black.svg';
import logoLight from '../../assets/brand/logo-light.svg';
import logoDark from '../../assets/brand/logo-dark.svg';
import logoMonoWhite from '../../assets/brand/logo-monochrome-white.svg';
import logoMonoBlack from '../../assets/brand/logo-monochrome-black.svg';
import wordmarkDark from '../../assets/brand/wordmark-dark.svg';
import wordmarkLight from '../../assets/brand/wordmark-light.svg';

export type BrandTone = 'dark' | 'light' | 'mono-white' | 'mono-black';

export const BRAND = {
  name: manifest.name, // "VELIORA"
  product: manifest.product, // "VELIORA App"
  colors: manifest.colors,
};

export const SYMBOL: Record<BrandTone, string> = {
  dark: symbolDark,
  light: symbolLight,
  'mono-white': symbolMonoWhite,
  'mono-black': symbolMonoBlack,
};

export const LOCKUP: Record<BrandTone, string> = {
  dark: logoDark,
  light: logoLight,
  'mono-white': logoMonoWhite,
  'mono-black': logoMonoBlack,
};

export const WORDMARK: Record<BrandTone, string> = {
  dark: wordmarkDark,
  light: wordmarkLight,
  'mono-white': wordmarkDark,
  'mono-black': wordmarkLight,
};

/** Ratios derived from the generated geometry (see scripts/brand). */
const sym = manifest.symbol;
export const METRICS = {
  /** symbol file height ÷ symbol ink height */
  symbolBoxPerInk: sym.box.h / sym.ink.h,
  symbolAspect: sym.box.w / sym.box.h,
  /** wordmark file height ÷ cap height */
  wordmarkBoxPerCap: (manifest.wordmark.cap + manifest.wordmark.pad * 2) / manifest.wordmark.cap,
  wordmarkAspect:
    (manifest.wordmark.width + manifest.wordmark.pad * 2) /
    (manifest.wordmark.cap + manifest.wordmark.pad * 2),
};
