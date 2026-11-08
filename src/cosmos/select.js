// The galaxy's planet picker (Alex: "a main planet you are hovering and then you move left and
// right between planets to be able to attach them easily"). From the selected planet, the next
// one to the left or right on screen, wrapping round at the ends. Pure, so it can be tested.
export function sideNeighbour(items, current, dir) {
  if (!items.length) return null;
  const sorted = [...items].sort((a, b) => a.x - b.x);
  const i = sorted.findIndex((it) => it.p === current);
  if (i < 0) return (dir > 0 ? sorted[0] : sorted[sorted.length - 1]).p;
  return sorted[(i + (dir > 0 ? 1 : -1) + sorted.length) % sorted.length].p;
}
