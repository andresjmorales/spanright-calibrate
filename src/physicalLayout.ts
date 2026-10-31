import type { CalibrationResult, Monitor } from "./types";

/**
 * Physical placement of calibrated monitors, in inches. Mirrors
 * `compute_physical_placements` in src-tauri/src/export/mod.rs — keep the two in sync.
 */
export interface PhysicalPlacement {
  monitor: Monitor;
  ppi: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Known PPIs (EDID / guessed / manual), with gaps filled in through the
 * calibration chain. `r.scale` is the pairwise ratio ppi_unbound / ppi_bound.
 */
export function derivePpi(
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

/**
 * Place the reference (uncalibrated) monitor at (0, 0) and every calibrated
 * monitor relative to the monitor it was bound to. Returns null when no
 * reference monitor with a known PPI exists.
 */
export function computePhysicalPlacements(
  monitors: Monitor[],
  results: CalibrationResult[]
): PhysicalPlacement[] | null {
  const ppiMap = derivePpi(monitors, results);
  const byId = new Map(monitors.map((m) => [m.id, m]));

  const calibratedIds = new Set(results.map((r) => r.monitorId));
  const refMon = monitors.find(
    (m) => !calibratedIds.has(m.id) && ppiMap.has(m.id)
  );
  if (!refMon) return null;

  const refPpi = ppiMap.get(refMon.id)!;
  const placed = new Map<number, PhysicalPlacement>();
  placed.set(refMon.id, {
    monitor: refMon,
    ppi: refPpi,
    x: 0,
    y: 0,
    w: refMon.resolutionX / refPpi,
    h: refMon.resolutionY / refPpi,
  });

  // Results are in calibration order, so each bound monitor is placed before
  // the monitors bound to it.
  for (const r of results) {
    const bound = placed.get(r.boundTo);
    const mUnbound = byId.get(r.monitorId);
    const ppiUnbound = ppiMap.get(r.monitorId);
    if (!bound || !mUnbound || ppiUnbound == null) continue;

    const mBound = bound.monitor;
    const ppiBound = bound.ppi;
    const w = mUnbound.resolutionX / ppiUnbound;
    const h = mUnbound.resolutionY / ppiUnbound;
    // The alignment midpoints sit at the same physical height (or x), so:
    // topUnbound = topBound + alignBound/ppiBound - alignUnbound/ppiUnbound
    const offsetIn =
      r.alignOffsetBound / ppiBound - r.alignOffsetUnbound / ppiUnbound;

    let x: number, y: number;
    if (r.bindHorizontal) {
      // The gap is measured in pixels of the right-hand monitor.
      const unboundFirst = mUnbound.positionX < mBound.positionX;
      const gapIn = Math.abs(r.gap) / (unboundFirst ? ppiBound : ppiUnbound);
      x = unboundFirst ? bound.x - w - gapIn : bound.x + bound.w + gapIn;
      y = bound.y + offsetIn;
    } else {
      // The gap is measured in pixels of the bottom monitor.
      const unboundFirst = mUnbound.positionY < mBound.positionY;
      const gapIn = Math.abs(r.gap) / (unboundFirst ? ppiBound : ppiUnbound);
      y = unboundFirst ? bound.y - h - gapIn : bound.y + bound.h + gapIn;
      x = bound.x + offsetIn;
    }

    placed.set(r.monitorId, { monitor: mUnbound, ppi: ppiUnbound, x, y, w, h });
  }

  return Array.from(placed.values());
}
