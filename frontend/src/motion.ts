// Same HMCL Motion.EASE_IN_OUT_CUBIC_EMPHASIZED curve as src/motion.rs.
function cubic(t: number, x1: number, y1: number, x2: number, y2: number): number {
  const sample = (u: number, a: number, b: number) =>
    3 * (1 - u) ** 2 * u * a + 3 * (1 - u) * u * u * b + u ** 3;
  let low = 0,
    high = 1;
  for (let n = 0; n < 32; n++) {
    const mid = (low + high) / 2;
    if (sample(mid, x1, x2) < t) low = mid;
    else high = mid;
  }
  return sample((low + high) / 2, y1, y2);
}
export function emphasized(t: number): number {
  t = Math.max(0, Math.min(1, t));
  if (t === 0 || t === 1) return t;
  const mx = 0.166666,
    my = 0.4;
  return t < mx
    ? cubic(t / mx, 0.05 / mx, 0, 0.133333 / mx, 0.06 / my) * my
    : cubic(
        (t - mx) / (1 - mx),
        (0.208333 - mx) / (1 - mx),
        (0.82 - my) / (1 - my),
        (0.25 - mx) / (1 - mx),
        1,
      ) *
        (1 - my) +
        my;
}
export const emphasizedCss = `linear(${Array.from({ length: 101 }, (_, n) => `${emphasized(n / 100).toFixed(6)} ${n}%`).join(',')})`;
