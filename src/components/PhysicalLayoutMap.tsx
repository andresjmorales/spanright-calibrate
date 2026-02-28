import type { CalibrationResult, Monitor } from "../types";

interface Props {
  monitors: Monitor[];
  results: CalibrationResult[];
}

interface PhysicalRect {
  id: number;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  diagonal: string;
}

const COLORS = [
  { bg: "rgba(108,141,250,0.22)", border: "#6c8dfa", text: "#a8bffc" },
  { bg: "rgba(76,175,125,0.22)", border: "#4caf7d", text: "#8fd4ad" },
  { bg: "rgba(245,166,35,0.22)", border: "#f5a623", text: "#f5c96a" },
  { bg: "rgba(231,76,111,0.22)", border: "#e74c6f", text: "#f0a0b4" },
  { bg: "rgba(160,120,240,0.22)", border: "#a078f0", text: "#c4aef5" },
  { bg: "rgba(80,200,200,0.22)", border: "#50c8c8", text: "#90dede" },
];

function derivePpi(
  monitors: Monitor[],
  results: CalibrationResult[]
): Map<number, number> {
  const ppiMap = new Map<number, number>();

  for (const m of monitors) {
    if (m.ppi != null) ppiMap.set(m.id, m.ppi);
  }

  // Propagate PPI through calibration chain using scale ratios
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

function buildPhysicalLayout(
  monitors: Monitor[],
  results: CalibrationResult[]
): PhysicalRect[] | null {
  const ppiMap = derivePpi(monitors, results);
  if (ppiMap.size === 0) return null;

  const rects = new Map<number, PhysicalRect>();

  for (const m of monitors) {
    const ppi = ppiMap.get(m.id);
    if (!ppi) continue;
    rects.set(m.id, {
      id: m.id,
      name: m.friendlyName || m.monitorName || `Display ${m.id + 1}`,
      x: 0,
      y: 0,
      w: m.resolutionX / ppi,
      h: m.resolutionY / ppi,
      diagonal: `${Math.sqrt(
        (m.resolutionX / ppi) ** 2 + (m.resolutionY / ppi) ** 2
      ).toFixed(1)}"`,
    });
  }

  // Identify reference monitor (not in results as monitorId)
  const calibratedIds = new Set(results.map((r) => r.monitorId));
  const refMon = monitors.find((m) => !calibratedIds.has(m.id) && rects.has(m.id));
  if (!refMon) return null;

  // Reference starts at (0, 0) — already set
  const placed = new Set<number>([refMon.id]);

  // Place calibrated monitors relative to their bound monitors
  // Process in order, since earlier results bind to already-placed monitors
  for (const r of results) {
    const bound = rects.get(r.boundTo);
    const current = rects.get(r.monitorId);
    if (!bound || !current) continue;
    if (!placed.has(r.boundTo)) continue;

    const mUnbound = monitors.find((m) => m.id === r.monitorId)!;
    const mBound = monitors.find((m) => m.id === r.boundTo)!;
    const ppiBound = ppiMap.get(r.boundTo)!;
    const ppiUnbound = ppiMap.get(r.monitorId)!;

    if (r.bindHorizontal) {
      const gapInches = Math.abs(r.gap) / ppiBound;
      // Correct physical vertical offset: the alignment midpoints are at
      // the same physical height, so:
      // topUnbound = topBound + alignBound/ppiBound - alignUnbound/ppiUnbound
      const offsetInches =
        r.alignOffsetBound / ppiBound - r.alignOffsetUnbound / ppiUnbound;

      if (mUnbound.positionX < mBound.positionX) {
        current.x = bound.x - current.w - gapInches;
      } else {
        current.x = bound.x + bound.w + gapInches;
      }
      current.y = bound.y + offsetInches;
    } else {
      const gapInches = Math.abs(r.gap) / ppiBound;
      const offsetInches =
        r.alignOffsetBound / ppiBound - r.alignOffsetUnbound / ppiUnbound;

      if (mUnbound.positionY < mBound.positionY) {
        current.y = bound.y - current.h - gapInches;
      } else {
        current.y = bound.y + bound.h + gapInches;
      }
      current.x = bound.x + offsetInches;
    }

    placed.add(r.monitorId);
  }

  return Array.from(rects.values()).filter((r) => placed.has(r.id));
}

export default function PhysicalLayoutMap({ monitors, results }: Props) {
  const layout = buildPhysicalLayout(monitors, results);
  if (!layout || layout.length === 0) return null;

  const minX = Math.min(...layout.map((r) => r.x));
  const minY = Math.min(...layout.map((r) => r.y));
  const maxX = Math.max(...layout.map((r) => r.x + r.w));
  const maxY = Math.max(...layout.map((r) => r.y + r.h));
  const totalW = maxX - minX;
  const totalH = maxY - minY;

  const padding = 28;
  const maxContainerW = 640;
  const maxContainerH = 220;
  const pxPerInch = Math.min(
    (maxContainerW - padding * 2) / totalW,
    (maxContainerH - padding * 2) / totalH
  );

  const svgW = totalW * pxPerInch + padding * 2;
  const svgH = totalH * pxPerInch + padding * 2;

  return (
    <div>
      <div className="section-title">Physical Layout</div>
      <div className="layout-map-container physical-layout">
        <svg
          width={svgW}
          height={svgH}
          viewBox={`0 0 ${svgW} ${svgH}`}
          className="layout-map-svg"
        >
          {layout.map((r) => {
            const x = (r.x - minX) * pxPerInch + padding;
            const y = (r.y - minY) * pxPerInch + padding;
            const w = r.w * pxPerInch;
            const h = r.h * pxPerInch;
            const color = COLORS[r.id % COLORS.length]!;
            const fontSize = Math.min(w * 0.12, h * 0.22, 14);
            const numSize = Math.min(w * 0.25, h * 0.4, 30);

            return (
              <g key={r.id}>
                <rect
                  x={x}
                  y={y}
                  width={w}
                  height={h}
                  rx={3}
                  fill={color.bg}
                  stroke={color.border}
                  strokeWidth={1.5}
                />
                <text
                  x={x + w / 2}
                  y={y + h / 2 - fontSize * 0.6}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill={color.border}
                  fontSize={numSize}
                  fontWeight={700}
                  opacity={0.4}
                >
                  {r.id + 1}
                </text>
                <text
                  x={x + w / 2}
                  y={y + h / 2 + numSize * 0.3}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill={color.text}
                  fontSize={fontSize}
                  fontWeight={500}
                >
                  {r.name}
                </text>
                <text
                  x={x + w / 2}
                  y={y + h / 2 + numSize * 0.3 + fontSize * 1.2}
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill={color.text}
                  fontSize={fontSize * 0.85}
                  opacity={0.6}
                >
                  {r.diagonal}
                </text>
              </g>
            );
          })}

          {/* Gap annotations between adjacent monitors */}
          {results.map((r) => {
            const a = layout.find((l) => l.id === r.boundTo);
            const b = layout.find((l) => l.id === r.monitorId);
            if (!a || !b) return null;
            const ppiBound = monitors.find((m) => m.id === r.boundTo)?.ppi;
            if (!ppiBound) return null;
            const gapInches = Math.abs(r.gap) / ppiBound;
            if (gapInches < 0.05) return null;

            if (r.bindHorizontal) {
              const leftRect = a.x < b.x ? a : b;
              const rightRect = a.x < b.x ? b : a;
              const gapX1 = (leftRect.x + leftRect.w - minX) * pxPerInch + padding;
              const gapX2 = (rightRect.x - minX) * pxPerInch + padding;
              // Place above the shorter of the two monitors
              const topEdge = Math.min(
                (a.y - minY) * pxPerInch + padding,
                (b.y - minY) * pxPerInch + padding
              );
              const annotY = topEdge - 6;

              return (
                <g key={`gap-${r.monitorId}`} opacity={0.6}>
                  <line
                    x1={gapX1}
                    y1={annotY - 4}
                    x2={gapX1}
                    y2={annotY + 4}
                    stroke="#9aa0a6"
                    strokeWidth={1}
                  />
                  <line
                    x1={gapX1}
                    y1={annotY}
                    x2={gapX2}
                    y2={annotY}
                    stroke="#9aa0a6"
                    strokeWidth={1}
                    strokeDasharray="3,2"
                  />
                  <line
                    x1={gapX2}
                    y1={annotY - 4}
                    x2={gapX2}
                    y2={annotY + 4}
                    stroke="#9aa0a6"
                    strokeWidth={1}
                  />
                  <text
                    x={(gapX1 + gapX2) / 2}
                    y={annotY - 8}
                    textAnchor="middle"
                    fill="#9aa0a6"
                    fontSize={9}
                  >
                    {gapInches.toFixed(2)}"
                  </text>
                </g>
              );
            }
            return null;
          })}
        </svg>
      </div>
    </div>
  );
}
