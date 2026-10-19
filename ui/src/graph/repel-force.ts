// Отталкивание узлов для раскладки графа (фича 025, задача T028).
//
// Та же сила, что `forceManyBody` из d3-force (Barnes–Hut, `distanceMin` 1), но
// квадродерево лежит в типизированных массивах, обход идёт без замыканий, а
// ячейка, целиком дальше `distanceMax`, отсекается вместе с поддеревом: d3 такие
// ячейки обходит до листьев и лишь не применяет к ним силу. На 5 000 узлах это
// укладывает тик раскладки в кадр (research.md §13).
//
// Сила одинакова для всех узлов — раскладке другой не нужно.

import type { Force, SimulationNodeDatum } from "d3-force";

export interface RepelForce<N extends SimulationNodeDatum> extends Force<N, undefined> {
  /** Сила отталкивания одного узла; отрицательная — отталкивание, как в d3. */
  strength(value: number): this;
  /** Точность Barnes–Hut: ячейка шириной `w` на расстоянии `l` считается целиком при `w / l < theta`. */
  theta(value: number): this;
  /** Дальше этого расстояния узлы друг на друга не действуют. */
  distanceMax(value: number): this;
}

/** Больше стольких узлов в ячейке — делим её на четыре. */
const LEAF_SIZE = 8;
/** Глубже не делим: совпадающие узлы иначе дробили бы ячейку бесконечно. */
const MAX_DEPTH = 32;

export function forceRepel<N extends SimulationNodeDatum>(): RepelForce<N> {
  let nodes: N[] = [];
  let strength = -30;
  let theta2 = 0.81;
  let distanceMax2 = Infinity;

  let xs = new Float64Array(0);
  let ys = new Float64Array(0);
  /** Индексы узлов, переставленные так, что узлы каждой ячейки лежат подряд. */
  let order = new Int32Array(0);
  let scratch = new Int32Array(0);

  // Ячейки дерева: центр масс, число узлов, левый верхний угол и ширина квадрата,
  // диапазон узлов в `order` (у листа) либо первый ребёнок и число детей (у ветви).
  let capacity = 0;
  let cellX = new Float64Array(0);
  let cellY = new Float64Array(0);
  let cellCount = new Int32Array(0);
  let cellX0 = new Float64Array(0);
  let cellY0 = new Float64Array(0);
  let cellW = new Float64Array(0);
  let cellStart = new Int32Array(0);
  let cellEnd = new Int32Array(0);
  /** Первый ребёнок ветви; −1 у листа. */
  let cellChild = new Int32Array(0);
  let cellChildren = new Int32Array(0);
  let cells = 0;
  let stack = new Int32Array(0);

  const grow = (need: number): void => {
    if (need <= capacity) return;
    const next = Math.max(need, capacity * 2, 64);
    const f64 = (old: Float64Array<ArrayBuffer>): Float64Array<ArrayBuffer> => {
      const fresh = new Float64Array(next);
      fresh.set(old);
      return fresh;
    };
    const i32 = (old: Int32Array<ArrayBuffer>): Int32Array<ArrayBuffer> => {
      const fresh = new Int32Array(next);
      fresh.set(old);
      return fresh;
    };
    cellX = f64(cellX);
    cellY = f64(cellY);
    cellX0 = f64(cellX0);
    cellY0 = f64(cellY0);
    cellW = f64(cellW);
    cellCount = i32(cellCount);
    cellStart = i32(cellStart);
    cellEnd = i32(cellEnd);
    cellChild = i32(cellChild);
    cellChildren = i32(cellChildren);
    capacity = next;
  };

  const jiggle = (): number => (Math.random() - 0.5) * 1e-6;

  /** Записывает ячейку в `slot`, детей и глубже — начиная с `cells = free`. */
  const buildAt = (
    slot: number,
    free: number,
    start: number,
    end: number,
    x0: number,
    y0: number,
    w: number,
    depth: number,
  ): number => {
    let sx = 0;
    let sy = 0;
    for (let k = start; k < end; k += 1) {
      const i = order[k]!;
      sx += xs[i]!;
      sy += ys[i]!;
    }
    const count = end - start;
    cellX[slot] = sx / count;
    cellY[slot] = sy / count;
    cellCount[slot] = count;
    cellX0[slot] = x0;
    cellY0[slot] = y0;
    cellW[slot] = w;
    cellStart[slot] = start;
    cellEnd[slot] = end;
    cellChild[slot] = -1;
    cellChildren[slot] = 0;
    cells = free;
    if (count <= LEAF_SIZE || depth >= MAX_DEPTH) return slot;

    const half = w / 2;
    const mx = x0 + half;
    const my = y0 + half;
    const counts = [0, 0, 0, 0];
    for (let k = start; k < end; k += 1) {
      const i = order[k]!;
      counts[(xs[i]! >= mx ? 1 : 0) + (ys[i]! >= my ? 2 : 0)]! += 1;
    }
    const offsets = [start, start + counts[0]!, start + counts[0]! + counts[1]!, end - counts[3]!];
    const at = offsets.slice();
    for (let k = start; k < end; k += 1) {
      const i = order[k]!;
      const q = (xs[i]! >= mx ? 1 : 0) + (ys[i]! >= my ? 2 : 0);
      scratch[at[q]!] = i;
      at[q]! += 1;
    }
    order.set(scratch.subarray(start, end), start);

    let present = 0;
    for (let q = 0; q < 4; q += 1) if (counts[q]! > 0) present += 1;
    const first = cells;
    cells += present;
    grow(cells);
    cellChild[slot] = first;
    cellChildren[slot] = present;
    let child = first;
    for (let q = 0; q < 4; q += 1) {
      if (counts[q] === 0) continue;
      const childStart = offsets[q]!;
      buildAt(child, cells, childStart, childStart + counts[q]!, q & 1 ? mx : x0, q & 2 ? my : y0, half, depth + 1);
      child += 1;
    }
    return slot;
  };

  const force = ((alpha: number): void => {
    const n = nodes.length;
    if (n < 2) return;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (let i = 0; i < n; i += 1) {
      const node = nodes[i]!;
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      xs[i] = x;
      ys[i] = y;
      order[i] = i;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
    const w = Math.max(maxX - minX, maxY - minY, 1);
    cells = 0;
    grow(1);
    buildAt(0, 1, 0, n, minX, minY, w, 0);

    if (stack.length < cells) stack = new Int32Array(cells);
    const s = strength * alpha;
    for (let i = 0; i < n; i += 1) {
      const xi = xs[i]!;
      const yi = ys[i]!;
      let vx = 0;
      let vy = 0;
      let top = 0;
      stack[top++] = 0;
      while (top > 0) {
        const c = stack[--top]!;
        // Отсечение: ближайшая точка квадрата ячейки дальше `distanceMax`.
        if (distanceMax2 !== Infinity) {
          const x0 = cellX0[c]!;
          const y0 = cellY0[c]!;
          const cw = cellW[c]!;
          const ox = xi < x0 ? x0 - xi : xi > x0 + cw ? xi - x0 - cw : 0;
          const oy = yi < y0 ? y0 - yi : yi > y0 + cw ? yi - y0 - cw : 0;
          if (ox * ox + oy * oy >= distanceMax2) continue;
        }
        let dx = cellX[c]! - xi;
        let dy = cellY[c]! - yi;
        let l = dx * dx + dy * dy;
        const cw = cellW[c]!;
        if ((cw * cw) / theta2 < l) {
          if (l < distanceMax2) {
            if (l < 1) l = Math.sqrt(l);
            const k = (s * cellCount[c]!) / l;
            vx += dx * k;
            vy += dy * k;
          }
          continue;
        }
        const child = cellChild[c]!;
        if (child >= 0) {
          const end = child + cellChildren[c]!;
          for (let d = child; d < end; d += 1) stack[top++] = d;
          continue;
        }
        const last = cellEnd[c]!;
        for (let k = cellStart[c]!; k < last; k += 1) {
          const j = order[k]!;
          if (j === i) continue;
          dx = xs[j]! - xi;
          dy = ys[j]! - yi;
          if (dx === 0) dx = jiggle();
          if (dy === 0) dy = jiggle();
          l = dx * dx + dy * dy;
          if (l >= distanceMax2) continue;
          if (l < 1) l = Math.sqrt(l);
          vx += (dx * s) / l;
          vy += (dy * s) / l;
        }
      }
      const node = nodes[i]!;
      node.vx = (node.vx ?? 0) + vx;
      node.vy = (node.vy ?? 0) + vy;
    }
  }) as RepelForce<N>;

  force.initialize = (next: N[]): void => {
    nodes = next;
    const n = nodes.length;
    xs = new Float64Array(n);
    ys = new Float64Array(n);
    order = new Int32Array(n);
    scratch = new Int32Array(n);
  };
  force.strength = (value: number) => {
    strength = value;
    return force;
  };
  force.theta = (value: number) => {
    theta2 = value * value;
    return force;
  };
  force.distanceMax = (value: number) => {
    distanceMax2 = value * value;
    return force;
  };
  return force;
}
