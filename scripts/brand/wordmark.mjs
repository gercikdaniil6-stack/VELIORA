// VELIORA wordmark — drawn geometrically (no font dependency).
// Cap height 100, monoline stroke, wide tracking; all sub-paths wound the same
// way so overlapping parts union cleanly under the nonzero fill rule.

const CAP = 100;
const S = 8.6; // stem weight
const f = (n) => +n.toFixed(2);

// Clockwise polygon → path
const poly = (pts) => 'M' + pts.map(([x, y]) => `${f(x)} ${f(y)}`).join(' L') + ' Z';
const rect = (x, y, w, h) => poly([[x, y], [x + w, y], [x + w, y + h], [x, y + h]]);

// Horizontal thickness of a diagonal stroke of weight S running dx over dy.
const hThick = (dx, dy) => S / Math.cos(Math.atan(dx / dy));

const glyphs = {
  V(x) {
    const W = 84, sh = hThick(W / 2, CAP);
    const innerY = CAP * (W / 2 - sh) / (W / 2);
    return { w: W, d: poly([[x, 0], [x + sh, 0], [x + W / 2, innerY], [x + W - sh, 0], [x + W, 0], [x + W / 2 + 1.1, CAP + 1.5], [x + W / 2 - 1.1, CAP + 1.5]]) };
  },
  E(x) {
    const W = 56;
    return { w: W, d: [rect(x, 0, S, CAP), rect(x, 0, W, S), rect(x, CAP / 2 - S / 2 - 1, W - 5, S), rect(x, CAP - S, W, S)].join(' ') };
  },
  L(x) {
    const W = 50;
    return { w: W, d: [rect(x, 0, S, CAP), rect(x, CAP - S, W, S)].join(' ') };
  },
  I(x) {
    return { w: S, d: rect(x, 0, S, CAP) };
  },
  O(x) {
    // overshoot 1.8 top & bottom; curves 6% heavier than stems (optical)
    const R = CAP / 2 + 1.8, r = R - S * 1.06, cx = x + R, cy = CAP / 2;
    const ring = (rad, sweep) =>
      `M${f(cx)} ${f(cy - rad)} A${f(rad)} ${f(rad)} 0 1 ${sweep} ${f(cx)} ${f(cy + rad)} A${f(rad)} ${f(rad)} 0 1 ${sweep} ${f(cx)} ${f(cy - rad)} Z`;
    return { w: R * 2, d: ring(R, 1) + ' ' + ring(r, 0) };
  },
  R(x) {
    const bowlH = 54, Ro = bowlH / 2, Ri = Ro - S * 1.04, bx = x + 32; // bowl centre x
    const bowl =
      `M${f(x)} 0 L${f(bx)} 0 A${f(Ro)} ${f(Ro)} 0 0 1 ${f(bx)} ${bowlH} L${f(x)} ${bowlH} ` +
      `L${f(x)} ${f(bowlH - S)} L${f(bx)} ${f(bowlH - S)} A${f(Ri)} ${f(Ri)} 0 0 0 ${f(bx)} ${f(S)} L${f(x)} ${f(S)} Z`;
    // leg: from the bowl junction to the baseline
    const x0 = x + 26, y0 = bowlH - S, x1 = x + 64;
    const sh = hThick(x1 - x0, CAP - y0);
    const leg = poly([[x0, y0], [x0 + sh, y0], [x1, CAP], [x1 - sh, CAP]]);
    return { w: 64, d: [rect(x, 0, S, CAP), bowl, leg].join(' ') };
  },
  A(x) {
    const W = 88, sh = hThick(W / 2, CAP);
    const apexIn = CAP * (sh / (W / 2));
    const outer = poly([[x + W / 2 - 1.1, -1.5], [x + W / 2 + 1.1, -1.5], [x + W, CAP], [x + W - sh, CAP], [x + W / 2, apexIn], [x + sh, CAP], [x, CAP]]);
    // crossbar spans between the stroke mid-lines at its height
    const yb = 64, mid = (y) => (W / 2) * (y / CAP);
    const bar = rect(x + W / 2 - mid(yb + S / 2), yb, mid(yb + S / 2) * 2, S * 0.94);
    return { w: W, d: outer + ' ' + bar };
  },
};

// Optical spacing (pair gaps, in cap-height units). Base tracking ≈ 0.44 em;
// open-sided pairs (L→I) tightened, round O given a touch less.
const TRACK = 44;
const PAIR = { VE: 40, EL: TRACK, LI: 32, IO: 40, OR: 40, RA: 36 };

export function wordmark(text = 'VELIORA') {
  let x = 0;
  const parts = [];
  for (let i = 0; i < text.length; i++) {
    const g = glyphs[text[i]](x);
    parts.push(g.d);
    x += g.w;
    if (i < text.length - 1) x += PAIR[text[i] + text[i + 1]] ?? TRACK;
  }
  return { d: parts.join(' '), width: Math.ceil(x), height: CAP };
}
