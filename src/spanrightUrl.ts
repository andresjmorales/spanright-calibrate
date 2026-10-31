import LZString from "lz-string";
import type { CalibrationResult, Monitor } from "./types";
import { computePhysicalPlacements } from "./physicalLayout";

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

interface UrlWindowsPosition {
  px: number;
  py: number;
}

interface UrlLayout {
  v: 1;
  m: UrlMonitor[];
  wp?: UrlWindowsPosition[];
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

export const SPANRIGHT_BASE_URL =
  import.meta.env.VITE_SPANRIGHT_URL || "https://spanright.com";

export function buildSpanrightUrl(
  monitors: Monitor[],
  results: CalibrationResult[],
  includeVirtualLayout = false
): string | null {
  const placed = computePhysicalPlacements(monitors, results);
  if (!placed) return null;

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
    const m = p.monitor;
    const ppi = p.ppi;
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
    // No `rot`: orientation is already baked into rx/ry/ar and placement. See the
    // rotation note in the JSON exporter (src-tauri/src/export/mod.rs) for details.
    if (m.friendlyName) entry.dn = m.friendlyName;
    return entry;
  });

  const layout: UrlLayout = { v: 1, m: urlMonitors };

  if (includeVirtualLayout) {
    layout.wp = placed.map((p) => ({
      px: p.monitor.positionX,
      py: p.monitor.positionY,
    }));
  }

  const json = JSON.stringify(layout);
  const compressed = LZString.compressToEncodedURIComponent(json);
  const encoded = compressed
    ? LAYOUT_ENCODING_LZ_PREFIX + compressed
    : json;

  return `${SPANRIGHT_BASE_URL}/#layout=${encoded}`;
}
