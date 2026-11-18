// VELIORA symbol geometry — single source of truth.
// Grid: 512 × 512. Two glass ribbons converge into one rounded point;
// their intersection (the "lens") is the connection itself.
//
// Key construction points (keep these when editing):
//   notch N  ≈ (258, 252)  — where the two inner edges meet
//   base     ≈ (256, 430)  — lowest point of the V
//   ink box  ≈ x 60…458, y 104…431

export const VIEW = 512;

// Right ribbon (lavender) — sits behind; its lower end tucks under the left ribbon.
export const RIGHT =
  'M318 110 C362 106 410 104 448 106 C455 106 458 111 456 118 ' +
  'C432 196 400 276 362 340 C354 354 346 367 337 377 ' +
  'C308 398 264 398 240 380 C228 346 240 292 258 252 ' +
  'C268 208 288 150 318 110 Z';

// Left ribbon (ice blue → deep navy) — sweeps down, wraps the base and its
// tail rises into the lavender ribbon.
export const LEFT =
  'M60 114 C96 107 132 104 160 105 C186 106 204 120 216 142 ' +
  'C232 172 248 214 258 252 C272 298 310 334 356 344 ' +
  'C364 346 365 356 361 364 C340 408 300 432 256 430 ' +
  'C222 428 198 410 182 378 C146 304 100 196 60 114 Z';

// Fold: the lighter inner face of the left ribbon (the twist from the reference).
export const FOLD =
  'M60 114 C96 107 132 104 160 105 C186 106 204 120 216 142 ' +
  'C230 168 244 204 254 236 C222 198 178 180 138 176 ' +
  'C102 172 78 150 60 114 Z';

// Visual ink bounds (used for lockups / icon centring).
export const INK = { x0: 60, y0: 104, x1: 458, y1: 431 };
