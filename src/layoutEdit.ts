import type { Monitor } from "./types";

export interface Pos {
  x: number;
  y: number;
}

export type Positions = Record<number, Pos>;

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function positionsOf(monitors: Monitor[]): Positions {
  const out: Positions = {};
  for (const m of monitors) out[m.id] = { x: m.positionX, y: m.positionY };
  return out;
}

export function samePositions(a: Positions, b: Positions): boolean {
  const ka = Object.keys(a);
  if (ka.length !== Object.keys(b).length) return false;
  return ka.every((k) => {
    const pa = a[+k]!;
    const pb = b[+k];
    return pb != null && pa.x === pb.x && pa.y === pb.y;
  });
}

/**
 * Move one monitor by (dx, dy). Windows keeps the primary monitor at (0, 0), so
 * moving the primary shifts every other monitor the opposite way instead.
 */
export function moveMonitor(
  monitors: Monitor[],
  positions: Positions,
  id: number,
  dx: number,
  dy: number
): Positions {
  const primary = monitors.find((m) => m.isPrimary);
  const out: Positions = { ...positions };
  if (primary && primary.id === id) {
    for (const m of monitors) {
      if (m.id === id) continue;
      const p = out[m.id]!;
      out[m.id] = { x: p.x - dx, y: p.y - dy };
    }
  } else {
    const p = out[id]!;
    out[id] = { x: p.x + dx, y: p.y + dy };
  }
  return out;
}

function rectOf(m: Monitor, positions: Positions): Rect {
  const p = positions[m.id]!;
  return { x: p.x, y: p.y, w: m.resolutionX, h: m.resolutionY };
}

function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

function sharesEdge(a: Rect, b: Rect): boolean {
  const xOverlap = Math.max(a.x, b.x) < Math.min(a.x + a.w, b.x + b.w);
  const yOverlap = Math.max(a.y, b.y) < Math.min(a.y + a.h, b.y + b.h);
  const touchX = a.x + a.w === b.x || b.x + b.w === a.x;
  const touchY = a.y + a.h === b.y || b.y + b.h === a.y;
  return (touchX && yOverlap) || (touchY && xOverlap);
}

/**
 * Mirrors monitors::layout::validate_layout in Rust: no overlaps, and every
 * monitor reachable from the primary through shared edges. Returns an error
 * message, or null if Windows should accept the layout unchanged.
 */
export function validateLayout(
  monitors: Monitor[],
  positions: Positions
): string | null {
  const rects = monitors.map((m) => rectOf(m, positions));
  for (let i = 0; i < rects.length; i++) {
    for (let j = i + 1; j < rects.length; j++) {
      if (overlaps(rects[i]!, rects[j]!)) {
        return `Displays ${monitors[i]!.id + 1} and ${monitors[j]!.id + 1} overlap`;
      }
    }
  }
  const start = Math.max(0, monitors.findIndex((m) => m.isPrimary));
  const reached = rects.map((_, i) => i === start);
  const stack = [start];
  while (stack.length) {
    const i = stack.pop()!;
    rects.forEach((r, j) => {
      if (!reached[j] && sharesEdge(rects[i]!, r)) {
        reached[j] = true;
        stack.push(j);
      }
    });
  }
  const lonely = reached.indexOf(false);
  if (lonely >= 0) {
    return `Display ${monitors[lonely]!.id + 1} must share an edge with another display`;
  }
  return null;
}

/**
 * Describe how the selected monitor lines up with each neighbour it shares an
 * edge with, e.g. "Top edge 137 px below Display 1". This is the number Windows
 * Settings never shows you.
 */
export function edgeOffsets(
  monitors: Monitor[],
  positions: Positions,
  id: number
): string[] {
  const sel = monitors.find((m) => m.id === id);
  if (!sel) return [];
  const a = rectOf(sel, positions);
  const out: string[] = [];
  for (const m of monitors) {
    if (m.id === id) continue;
    const b = rectOf(m, positions);
    if (!sharesEdge(a, b)) continue;
    const name = `Display ${m.id + 1}`;
    const sideBySide = a.x + a.w === b.x || b.x + b.w === a.x;
    if (sideBySide) {
      const d = a.y - b.y;
      out.push(
        d === 0
          ? `Top edge aligned with ${name}`
          : `Top edge ${Math.abs(d)} px ${d > 0 ? "below" : "above"} ${name}`
      );
    } else {
      const d = a.x - b.x;
      out.push(
        d === 0
          ? `Left edge aligned with ${name}`
          : `Left edge ${Math.abs(d)} px ${d > 0 ? "right of" : "left of"} ${name}`
      );
    }
  }
  return out;
}
