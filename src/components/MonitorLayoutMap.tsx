import type { Monitor } from "../types";

interface Props {
  monitors: Monitor[];
}

const COLORS = [
  { bg: "rgba(108,141,250,0.18)", border: "#6c8dfa", text: "#a8bffc" },
  { bg: "rgba(76,175,125,0.18)", border: "#4caf7d", text: "#8fd4ad" },
  { bg: "rgba(245,166,35,0.18)", border: "#f5a623", text: "#f5c96a" },
  { bg: "rgba(231,76,111,0.18)", border: "#e74c6f", text: "#f0a0b4" },
  { bg: "rgba(160,120,240,0.18)", border: "#a078f0", text: "#c4aef5" },
  { bg: "rgba(80,200,200,0.18)", border: "#50c8c8", text: "#90dede" },
];

export default function MonitorLayoutMap({ monitors }: Props) {
  if (monitors.length === 0) return null;

  const minX = Math.min(...monitors.map((m) => m.positionX));
  const minY = Math.min(...monitors.map((m) => m.positionY));
  const maxX = Math.max(...monitors.map((m) => m.positionX + m.resolutionX));
  const maxY = Math.max(...monitors.map((m) => m.positionY + m.resolutionY));
  const totalW = maxX - minX;
  const totalH = maxY - minY;

  const padding = 12;
  const maxContainerW = 600;
  const maxContainerH = 160;
  const scaleX = (maxContainerW - padding * 2) / totalW;
  const scaleY = (maxContainerH - padding * 2) / totalH;
  const scale = Math.min(scaleX, scaleY);

  const svgW = totalW * scale + padding * 2;
  const svgH = totalH * scale + padding * 2;

  return (
    <div className="layout-map-container">
      <svg
        width={svgW}
        height={svgH}
        viewBox={`0 0 ${svgW} ${svgH}`}
        className="layout-map-svg"
      >
        {monitors.map((m, i) => {
          const x = (m.positionX - minX) * scale + padding;
          const y = (m.positionY - minY) * scale + padding;
          const w = m.resolutionX * scale;
          const h = m.resolutionY * scale;
          const color = COLORS[i % COLORS.length];

          const label = m.friendlyName || `Display ${m.id + 1}`;
          const fontSize = Math.min(w * 0.14, h * 0.28, 16);
          const numSize = Math.min(w * 0.3, h * 0.5, 36);

          return (
            <g key={m.id}>
              <rect
                x={x}
                y={y}
                width={w}
                height={h}
                rx={4}
                fill={color.bg}
                stroke={color.border}
                strokeWidth={1.5}
              />
              <text
                x={x + w / 2}
                y={y + h / 2 - fontSize * 0.3}
                textAnchor="middle"
                dominantBaseline="central"
                fill={color.border}
                fontSize={numSize}
                fontWeight={700}
                opacity={0.5}
              >
                {m.id + 1}
              </text>
              <text
                x={x + w / 2}
                y={y + h / 2 + numSize * 0.35}
                textAnchor="middle"
                dominantBaseline="central"
                fill={color.text}
                fontSize={fontSize}
                fontWeight={500}
              >
                {label}
              </text>
              <text
                x={x + w / 2}
                y={y + h / 2 + numSize * 0.35 + fontSize * 1.2}
                textAnchor="middle"
                dominantBaseline="central"
                fill={color.text}
                fontSize={fontSize * 0.8}
                opacity={0.6}
              >
                {m.resolutionX}x{m.resolutionY}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}
