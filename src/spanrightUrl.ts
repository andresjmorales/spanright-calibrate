import LZString from "lz-string";
import type { CalibrationResult, Monitor } from "./types";

const LAYOUT_ENCODING_LZ_PREFIX = "~";

interface UrlMonitor {
  n: string;
  d: number;
  ar: [number, number];
  rx: number;
  ry: number;
  x: number;
  y: number;
  rot?: 90;
  dn?: string;
}

interface UrlLayout {
  v: 1;
  m: UrlMonitor[];
}

function gcd(a: number, b: number): number {
  while (b) {
    [a, b] = [b, a % b];
  }
  return a;
}

function aspectRatio(rx: number, ry: number): [number, number] {
  const g = gcd(rx, ry);
  return g ? [rx / g, ry / g] : [16, 9];
}

function round4(v: number): number {
  return Math.round(v * 10000) / 10000;
}

function derivePpi(
  monitors: Monitor[],
  results: CalibrationResult[]
): Map<number, number> {
  const ppiMap = new Map<number, number>();
  for (const m of monitors) {
    if (m.ppi != null) ppiMap.set(m.id, m.ppi);
  }
  let changed = true;
  while (changed) {
    changed = false;
    for (const r of results) {
      if (!ppiMap.has(r.monitorId) && ppiMap.has(r.boundTo)) {
        ppiMap.set(r.monitorId, ppiMap.get(r.boundTo)! * r.scale);
        changed = true;
      }
      if (!ppiMap.has(r.boundTo) && ppiMap.has(r.monitorId)) {
        ppiMap.set(r.boundTo, ppiMap.get(r.monitorId)! / r.scale);
        changed = true;
      }
    }
  }
  return ppiMap;
}

function formatResolution(rx: number, ry: number): string {
  const map: Record<string, string> = {
    "1920x1080": "FHD",
    "1920x1200": "WUXGA",
    "2560x1080": "UWFHD",
    "2560x1440": "QHD",
    "3440x1440": "UWQHD",
    "3840x2160": "4K",
    "3840x1600": "UW4K",
  };
  return map[`${rx}x${ry}`] || `${rx}x${ry}`;
}

export function buildSpanrightUrl(
  monitors: Monitor[],
  results: CalibrationResult[]
): string | null {
  const ppiMap = derivePpi(monitors, results);
  if (ppiMap.size === 0) return null;

  // Build physical placements (same logic as PhysicalLayoutMap)
  const positions = new Map<number, { x: number; y: number }>();
  const calibratedIds = new Set(results.map((r) => r.monitorId));
  const refMon = monitors.find(
    (m) => !calibratedIds.has(m.id) && ppiMap.has(m.id)
  );
  if (!refMon) return null;

  positions.set(refMon.id, { x: 0, y: 0 });

  for (const r of results) {
    if (!positions.has(r.boundTo)) continue;
    const ppiBound = ppiMap.get(r.boundTo)!;
    const ppiUnbound = ppiMap.get(r.monitorId)!;
    const bound = positions.get(r.boundTo)!;
    const mUnbound = monitors.find((m) => m.id === r.monitorId)!;
    const mBound = monitors.find((m) => m.id === r.boundTo)!;
    const w = mUnbound.resolutionX / ppiUnbound;
    const h = mUnbound.resolutionY / ppiUnbound;

    let x: number, y: number;
    if (r.bindHorizontal) {
      const gapIn = Math.abs(r.gap) / ppiBound;
      const offsetIn =
        r.alignOffsetBound / ppiBound - r.alignOffsetUnbound / ppiUnbound;
      const boundW = mBound.resolutionX / ppiBound;
      x =
        mUnbound.positionX < mBound.positionX
          ? bound.x - w - gapIn
          : bound.x + boundW + gapIn;
      y = bound.y + offsetIn;
    } else {
      const gapIn = Math.abs(r.gap) / ppiBound;
      const offsetIn =
        r.alignOffsetBound / ppiBound - r.alignOffsetUnbound / ppiUnbound;
      const boundH = mBound.resolutionY / ppiBound;
      y =
        mUnbound.positionY < mBound.positionY
          ? bound.y - h - gapIn
          : bound.y + boundH + gapIn;
      x = bound.x + offsetIn;
    }
    positions.set(r.monitorId, { x, y });
  }

  // Center on Spanright's 144"×96" canvas
  const placed = Array.from(positions.entries())
    .map(([id, pos]) => {
      const ppi = ppiMap.get(id)!;
      const m = monitors.find((mon) => mon.id === id)!;
      return { id, ...pos, w: m.resolutionX / ppi, h: m.resolutionY / ppi };
    });

  if (placed.length === 0) return null;

  const minX = Math.min(...placed.map((p) => p.x));
  const maxX = Math.max(...placed.map((p) => p.x + p.w));
  const minY = Math.min(...placed.map((p) => p.y));
  const maxY = Math.max(...placed.map((p) => p.y + p.h));
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  const offsetX = 72 - cx;
  const offsetY = 48 - cy;

  const urlMonitors: UrlMonitor[] = placed.map((p) => {
    const m = monitors.find((mon) => mon.id === p.id)!;
    const ppi = ppiMap.get(p.id)!;
    const diagonal = Math.sqrt(
      (m.resolutionX / ppi) ** 2 + (m.resolutionY / ppi) ** 2
    );
    const entry: UrlMonitor = {
      n: `${Math.round(diagonal)}" ${formatResolution(m.resolutionX, m.resolutionY)}`,
      d: Math.round(diagonal * 100) / 100,
      ar: aspectRatio(m.resolutionX, m.resolutionY),
      rx: m.resolutionX,
      ry: m.resolutionY,
      x: round4(p.x + offsetX),
      y: round4(p.y + offsetY),
    };
    if (m.orientation === 1) entry.rot = 90;
    if (m.friendlyName) entry.dn = m.friendlyName;
    return entry;
  });

  const layout: UrlLayout = { v: 1, m: urlMonitors };
  const json = JSON.stringify(layout);
  const compressed = LZString.compressToEncodedURIComponent(json);
  const encoded = compressed
    ? LAYOUT_ENCODING_LZ_PREFIX + compressed
    : json;

  return `https://spanright.com/#layout=${encoded}`;
}
