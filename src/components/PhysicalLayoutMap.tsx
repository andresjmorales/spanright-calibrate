import type { CalibrationResult, Monitor } from "../types";
import { computePhysicalPlacements } from "../physicalLayout";

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

function buildPhysicalLayout(
  monitors: Monitor[],
  results: CalibrationResult[]
): PhysicalRect[] | null {
  const placements = computePhysicalPlacements(monitors, results);
  if (!placements) return null;
  return placements.map(({ monitor: m, x, y, w, h }) => ({
    id: m.id,
    name: m.friendlyName || m.monitorName || `Display ${m.id + 1}`,
    x,
    y,
    w,
    h,
    diagonal: `${Math.sqrt(w ** 2 + h ** 2).toFixed(1)}"`,
  }));
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

            if (r.bindHorizontal) {
              const leftRect = a.x < b.x ? a : b;
              const rightRect = a.x < b.x ? b : a;
              const gapInches = rightRect.x - (leftRect.x + leftRect.w);
              if (gapInches < 0.05) return null;
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

            // Stacked pair: annotate the vertical gap to the left of the two monitors.
            const topRect = a.y < b.y ? a : b;
            const bottomRect = a.y < b.y ? b : a;
            const gapInches = bottomRect.y - (topRect.y + topRect.h);
            if (gapInches < 0.05) return null;
            const gapY1 = (topRect.y + topRect.h - minY) * pxPerInch + padding;
            const gapY2 = (bottomRect.y - minY) * pxPerInch + padding;
            const leftEdge = Math.min(
              (a.x - minX) * pxPerInch + padding,
              (b.x - minX) * pxPerInch + padding
            );
            const annotX = leftEdge - 6;
            const midY = (gapY1 + gapY2) / 2;

            return (
              <g key={`gap-${r.monitorId}`} opacity={0.6}>
                <line x1={annotX - 4} y1={gapY1} x2={annotX + 4} y2={gapY1} stroke="#9aa0a6" strokeWidth={1} />
                <line
                  x1={annotX}
                  y1={gapY1}
                  x2={annotX}
                  y2={gapY2}
                  stroke="#9aa0a6"
                  strokeWidth={1}
                  strokeDasharray="3,2"
                />
                <line x1={annotX - 4} y1={gapY2} x2={annotX + 4} y2={gapY2} stroke="#9aa0a6" strokeWidth={1} />
                <text
                  x={annotX - 8}
                  y={midY}
                  textAnchor="middle"
                  fill="#9aa0a6"
                  fontSize={9}
                  transform={`rotate(-90 ${annotX - 8} ${midY})`}
                >
                  {gapInches.toFixed(2)}"
                </text>
              </g>
            );
          })}
        </svg>
      </div>
    </div>
  );
}
