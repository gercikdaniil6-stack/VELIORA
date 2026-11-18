import type { CSSProperties } from 'react';
import { useThemeMode } from '../theme/engine';
import { BRAND, LOCKUP, METRICS, SYMBOL, WORDMARK, type BrandTone } from './brand';

/**
 * BrandLogo — the single component that renders the VELIORA identity.
 *
 * Sizing is by *ink height* (what the eye sees), never by file box, so a 24 px
 * symbol is exactly 24 px of ribbon regardless of the SVG's clear space.
 *
 *  variant="symbol"   — the mark alone
 *  variant="wordmark" — VELIORA lettering alone (size = cap height)
 *  variant="inline"   — mark + lettering for UI chrome (size = mark height;
 *                       cap = 0.42 × mark, gap = 0.36 × mark)
 *  variant="lockup"   — the master horizontal logo file (size = total height)
 */
export type BrandLogoVariant = 'symbol' | 'wordmark' | 'inline' | 'lockup';

const SIZES = { xs: 16, sm: 20, md: 28, lg: 48, xl: 96 } as const;
export type BrandLogoSize = keyof typeof SIZES | number;

interface Props {
  variant?: BrandLogoVariant;
  tone?: BrandTone;
  size?: BrandLogoSize;
  /** Adds the standard clear space (¼ of the mark height) around the logo. */
  clearSpace?: boolean;
  className?: string;
  style?: CSSProperties;
  /** Decorative logos are hidden from screen readers. */
  decorative?: boolean;
}

export function BrandLogo({
  variant = 'symbol',
  tone: toneProp,
  size = 'md',
  clearSpace = false,
  className,
  style,
  decorative = false,
}: Props) {
  // Without an explicit tone the logo follows the theme: light artwork on light themes.
  const mode = useThemeMode();
  const tone: BrandTone = toneProp ?? (mode === 'light' ? 'light' : 'dark');
  const px = typeof size === 'number' ? size : SIZES[size];
  const alt = decorative ? '' : BRAND.name;
  const wrap: CSSProperties = {
    display: 'inline-flex',
    alignItems: 'center',
    flex: 'none',
    lineHeight: 0,
    padding: clearSpace ? px / 4 : 0,
    ...style,
  };

  const symbol = (inkH: number) => {
    const h = inkH * METRICS.symbolBoxPerInk;
    return (
      <img
        src={SYMBOL[tone]}
        alt={alt}
        draggable={false}
        width={Math.round(h * METRICS.symbolAspect)}
        height={Math.round(h)}
        // compensate the file's built-in clear space so ink edges align with layout
        style={{ margin: -(h - inkH) / 2 + 'px ' + -(h - inkH) / 2 + 'px' }}
      />
    );
  };

  const wordmark = (cap: number) => {
    const h = cap * METRICS.wordmarkBoxPerCap;
    const pad = (h - cap) / 2;
    return (
      <img
        src={WORDMARK[tone]}
        alt={variant === 'wordmark' ? alt : ''}
        draggable={false}
        width={Math.round(h * METRICS.wordmarkAspect)}
        height={Math.round(h)}
        style={{ margin: `${-pad}px ${-pad}px` }}
      />
    );
  };

  if (variant === 'symbol') {
    return <span className={className} style={wrap} role={decorative ? undefined : 'img'}>{symbol(px)}</span>;
  }
  if (variant === 'wordmark') {
    return <span className={className} style={wrap}>{wordmark(px)}</span>;
  }
  if (variant === 'inline') {
    return (
      <span className={className} style={{ ...wrap, gap: px * 0.36 }} aria-label={decorative ? undefined : BRAND.name}>
        {symbol(px)}
        {wordmark(px * 0.42)}
      </span>
    );
  }
  return (
    <span className={className} style={wrap}>
      <img src={LOCKUP[tone]} alt={alt} draggable={false} height={px} style={{ height: px, width: 'auto' }} />
    </span>
  );
}
