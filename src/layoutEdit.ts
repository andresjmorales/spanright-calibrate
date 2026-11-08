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

/** An alignment the dragged monitor snapped to, drawn as a guide line. */
export interface SnapGuide {
  /** "x": vertical line at x = at; "y": horizontal line at y = at. */
  axis: "x" | "y";
  at: number;
}

interface Candidate {
  pos: Pos;
  /** Which coordinate slides along the edge; the other one is fixed. */
  slide: "x" | "y";
  free: [number, number][];
}

/** Remove the integer intervals in `blocked` from the integer interval [lo, hi]. */
function subtractIntervals(
  lo: number,
  hi: number,
  blocked: [number, number][]
): [number, number][] {
  let free: [number, number][] = lo <= hi ? [[lo, hi]] : [];
  for (const [bl, bh] of blocked) {
    const next: [number, number][] = [];
    for (const [fl, fh] of free) {
      if (bh < fl || bl > fh) {
        next.push([fl, fh]);
        continue;
      }
      if (fl < bl) next.push([fl, bl - 1]);
      if (bh < fh) next.push([bh + 1, fh]);
    }
    free = next;
  }
  return free;
}

function nearestIn(free: [number, number][], v: number): number | null {
  let best: number | null = null;
  for (const [l, h] of free) {
    const c = Math.min(h, Math.max(l, v));
    if (best == null || Math.abs(c - v) < Math.abs(best - v)) best = c;
  }
  return best;
}

/**
 * Find the valid position closest to `desired` for monitor `id`: flush against
 * an edge of another monitor (no gaps), not overlapping anything, and leaving
 * the whole layout connected. Because every side of every other monitor is a
 * candidate, dragging past a corner carries the monitor around onto the next
 * side. Within `snapDist` virtual pixels the sliding coordinate snaps to the
 * top/bottom/centre (or left/right/centre) of the other monitors.
 * Returns null if there is nowhere valid to put it.
 */
export function constrainPosition(
  monitors: Monitor[],
  positions: Positions,
  id: number,
  desired: Pos,
  snapDist = 0
): { pos: Pos; guide: SnapGuide | null } | null {
  const me = monitors.find((m) => m.id === id);
  if (!me) return null;
  const w = me.resolutionX;
  const h = me.resolutionY;
  const current = positions[id]!;
  const others = monitors.filter((m) => m.id !== id).map((m) => rectOf(m, positions));

  const candidates: Candidate[] = [];
  for (const b of others) {
    // Left / right of b: x fixed, y slides while keeping a shared edge.
    for (const x of [b.x - w, b.x + b.w]) {
      const blocked = others
        .filter((c) => x < c.x + c.w && c.x < x + w)
        .map((c) => [c.y - h + 1, c.y + c.h - 1] as [number, number]);
      const free = subtractIntervals(b.y - h + 1, b.y + b.h - 1, blocked);
      const y = nearestIn(free, desired.y);
      if (y != null) candidates.push({ pos: { x, y }, slide: "y", free });
    }
    // Above / below b: y fixed, x slides.
    for (const y of [b.y - h, b.y + b.h]) {
      const blocked = others
        .filter((c) => y < c.y + c.h && c.y < y + h)
        .map((c) => [c.x - w + 1, c.x + c.w - 1] as [number, number]);
      const free = subtractIntervals(b.x - w + 1, b.x + b.w - 1, blocked);
      const x = nearestIn(free, desired.x);
      if (x != null) candidates.push({ pos: { x, y }, slide: "x", free });
    }
  }

  const dist = (a: Pos, b: Pos) => Math.hypot(a.x - b.x, a.y - b.y);
  // Ties (e.g. at the end of an edge) prefer staying where we already are.
  candidates.sort(
    (a, b) =>
      dist(a.pos, desired) - dist(b.pos, desired) ||
      dist(a.pos, current) - dist(b.pos, current)
  );

  const valid = (p: Pos) => validateLayout(monitors, { ...positions, [id]: p }) === null;
  const best = candidates.find((c) => valid(c.pos));
  if (!best) return null;

  if (snapDist > 0) {
    const s = best.slide;
    const len = s === "y" ? h : w;
    const targets: { v: number; guide: number }[] = [];
    for (const c of others) {
      const start = s === "y" ? c.y : c.x;
      const size = s === "y" ? c.h : c.w;
      targets.push({ v: start, guide: start });
      targets.push({ v: start + size - len, guide: start + size });
      targets.push({ v: Math.round(start + (size - len) / 2), guide: start + size / 2 });
    }
    targets.sort((a, b) => Math.abs(a.v - best.pos[s]) - Math.abs(b.v - best.pos[s]));
    for (const t of targets) {
      if (Math.abs(t.v - best.pos[s]) > snapDist) break;
      if (!best.free.some(([l, hh]) => t.v >= l && t.v <= hh)) continue;
      const p = { ...best.pos, [s]: t.v };
      if (valid(p)) return { pos: p, guide: { axis: s, at: t.guide } };
    }
  }
  return { pos: best.pos, guide: null };
}

/**
 * Suggest a virtual layout from the calibrated physical layout: each calibrated
 * pair is put side by side (or stacked) the way it sits on the desk, offset so
 * the cursor crosses between them at the same physical height (or x) at the
 * middle of their shared edge. Exact everywhere along the edge is only possible
 * when both monitors have the same PPI. Returns null unless every monitor has
 * a physical placement.
 */
export function suggestFromPhysical(
  monitors: Monitor[],
  placements: { monitor: Monitor; ppi: number; x: number; y: number; w: number; h: number }[],
  results: { monitorId: number; boundTo: number; bindHorizontal: boolean }[]
): Positions | null {
  const phys = new Map(placements.map((p) => [p.monitor.id, p]));
  if (placements.length === 0 || monitors.some((m) => !phys.has(m.id))) return null;
  const byId = new Map(monitors.map((m) => [m.id, m]));

  const root = placements[0]!.monitor;
  const out: Positions = { [root.id]: { x: root.positionX, y: root.positionY } };
  for (const r of results) {
    const a = out[r.boundTo];
    const pa = phys.get(r.boundTo);
    const pb = phys.get(r.monitorId);
    const ma = byId.get(r.boundTo);
    const mb = byId.get(r.monitorId);
    if (!a || !pa || !pb || !ma || !mb) continue;

    if (r.bindHorizontal) {
      const x = pb.x < pa.x ? a.x - mb.resolutionX : a.x + ma.resolutionX;
      const mid = (Math.max(pa.y, pb.y) + Math.min(pa.y + pa.h, pb.y + pb.h)) / 2;
      const raw = Math.round(a.y + (mid - pa.y) * pa.ppi - (mid - pb.y) * pb.ppi);
      const y = Math.min(a.y + ma.resolutionY - 1, Math.max(a.y - mb.resolutionY + 1, raw));
      out[r.monitorId] = { x, y };
    } else {
      const y = pb.y < pa.y ? a.y - mb.resolutionY : a.y + ma.resolutionY;
      const mid = (Math.max(pa.x, pb.x) + Math.min(pa.x + pa.w, pb.x + pb.w)) / 2;
      const raw = Math.round(a.x + (mid - pa.x) * pa.ppi - (mid - pb.x) * pb.ppi);
      const x = Math.min(a.x + ma.resolutionX - 1, Math.max(a.x - mb.resolutionX + 1, raw));
      out[r.monitorId] = { x, y };
    }
  }
  if (monitors.some((m) => !out[m.id])) return null;

  const primary = monitors.find((m) => m.isPrimary) ?? monitors[0]!;
  const o = out[primary.id]!;
  const norm: Positions = {};
  for (const m of monitors) norm[m.id] = { x: out[m.id]!.x - o.x, y: out[m.id]!.y - o.y };
  return norm;
}
