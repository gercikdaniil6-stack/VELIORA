// Types for the React Bits SplashCursor (JS + CSS variant, source kept unmodified in SplashCursor.jsx).
import type { CSSProperties, FC } from 'react';

export interface SplashCursorProps {
  colors?: string[];
  rainbow?: boolean;
  colorSpeed?: number;
  intensity?: number;
  size?: number;
  force?: number;
  swirl?: number;
  trail?: number;
  momentum?: number;
  shading?: number;
  glow?: number;
  quality?: 'low' | 'medium' | 'high';
  clickSplash?: boolean;
  intro?: boolean;
  contained?: boolean;
  theme?: 'dark' | 'light';
  zIndex?: number;
  className?: string;
  style?: CSSProperties;
}

declare const SplashCursor: FC<SplashCursorProps>;
export default SplashCursor;
