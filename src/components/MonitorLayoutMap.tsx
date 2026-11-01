import { useMemo, useRef, useState } from "react";
import type { Monitor } from "../types";
import { applyMonitorPositions } from "../hooks/useTauriCommands";
import {
  edgeOffsets,
  moveMonitor,
  positionsOf,
  samePositions,
  validateLayout,
  type Positions,
} from "../layoutEdit";

interface Props {
  monitors: Monitor[];
  /** Called with the monitors as Windows reports them after a layout is applied. */
  onApplied: (monitors: Monitor[]) => void;
}

const COLORS = [
  { bg: "rgba(108,141,250,0.18)", border: "#6c8dfa", text: "#a8bffc" },
  { bg: "rgba(76,175,125,0.18)", border: "#4caf7d", text: "#8fd4ad" },
  { bg: "rgba(245,166,35,0.18)", border: "#f5a623", text: "#f5c96a" },
  { bg: "rgba(231,76,111,0.18)", border: "#e74c6f", text: "#f0a0b4" },
  { bg: "rgba(160,120,240,0.18)", border: "#a078f0", text: "#c4aef5" },
  { bg: "rgba(80,200,200,0.18)", border: "#50c8c8", text: "#90dede" },
];

const ARROWS: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

function PencilIcon() {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true">
      <path d="M11.013 1.427a1.75 1.75 0 012.474 0l1.086 1.086a1.75 1.75 0 010 2.474l-8.61 8.61c-.21.21-.47.364-.756.445l-3.251.93a.75.75 0 01-.927-.928l.929-3.25c.081-.286.235-.547.445-.758l8.61-8.61zm1.414 1.06a.25.25 0 00-.354 0L10.811 3.75l1.439 1.44 1.263-1.263a.25.25 0 000-.354l-1.086-1.086zM11.189 6.25L9.75 4.81l-6.286 6.287a.25.25 0 00-.064.108l-.558 1.953 1.953-.558a.249.249 0 00.108-.064l6.286-6.286z" />
    </svg>
  );
}

export default function MonitorLayoutMap({ monitors, onApplied }: Props) {
  const system = useMemo(() => positionsOf(monitors), [monitors]);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Positions>(system);
  const [selected, setSelected] = useState<number | null>(null);
  const [applying, setApplying] = useState(false);
  const [message, setMessage] = useState<{ text: string; error?: boolean } | null>(null);
  /** System positions from before the last Apply, so it can be undone. */
  const [undo, setUndo] = useState<Positions | null>(null);
  const mapRef = useRef<HTMLDivElement>(null);

  // Whenever Windows' layout changes (refresh, calibration, apply), start the
  // draft over from it so we never edit stale positions. Done during render
  // (not in an effect) so a draft missing a new monitor is never drawn.
  const [prevSystem, setPrevSystem] = useState(system);
  if (prevSystem !== system) {
    setPrevSystem(system);
    setDraft(system);
    return null;
  }

  if (monitors.length === 0) return null;

  const positions = editing ? draft : system;
  const dirty = editing && !samePositions(draft, system);
  const validation = editing ? validateLayout(monitors, draft) : null;

  const startEditing = () => {
    setDraft(system);
    setEditing(true);
    setSelected(monitors.find((m) => !m.isPrimary)?.id ?? monitors[0]!.id);
    setMessage(null);
    setUndo(null);
    setTimeout(() => mapRef.current?.focus(), 0);
  };

  const stopEditing = () => {
    setEditing(false);
    setDraft(system);
    setSelected(null);
    setMessage(null);
    setUndo(null);
  };

  const move = (dx: number, dy: number) => {
    if (selected == null) return;
    setDraft((d) => moveMonitor(monitors, d, selected, dx, dy));
    setMessage(null);
  };

  const apply = async (target: Positions, isUndo = false) => {
    setApplying(true);
    setMessage(null);
    const before = system;
    try {
      const result = await applyMonitorPositions(
        monitors.map((m) => ({ id: m.id, ...target[m.id]! }))
      );
      onApplied(result);
      setUndo(isUndo ? null : before);
      // Windows can still nudge a layout it doesn't like; compare against what
      // we asked for (normalized to the primary monitor at 0,0).
      const primary = monitors.find((m) => m.isPrimary);
      const o = primary ? target[primary.id]! : { x: 0, y: 0 };
      const asked: Positions = {};
      for (const m of monitors) {
        const p = target[m.id]!;
        asked[m.id] = { x: p.x - o.x, y: p.y - o.y };
      }
      const adjusted = !samePositions(asked, positionsOf(result));
      setMessage(
        adjusted
          ? { text: "Applied, but Windows adjusted the layout — check the positions below." }
          : { text: isUndo ? "Previous layout restored." : "Layout applied to Windows." }
      );
    } catch (e) {
      setMessage({ text: e instanceof Error ? e.message : String(e), error: true });
    } finally {
      setApplying(false);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!editing) return;
    const dir = ARROWS[e.key];
    if (dir) {
      e.preventDefault();
      const step = e.ctrlKey ? 100 : e.shiftKey ? 10 : 1;
      move(dir[0] * step, dir[1] * step);
      return;
    }
    const n = parseInt(e.key, 10);
    if (n >= 1 && n <= 9) {
      const m = monitors.find((mm) => mm.id === n - 1);
      if (m) setSelected(m.id);
    }
  };

  // Fit both the draft and the system layout so moved monitors' original
  // outlines stay visible.
  const all = editing
    ? monitors.flatMap((m) => [
        { x: draft[m.id]!.x, y: draft[m.id]!.y, m },
        { x: system[m.id]!.x, y: system[m.id]!.y, m },
      ])
    : monitors.map((m) => ({ x: system[m.id]!.x, y: system[m.id]!.y, m }));
  const minX = Math.min(...all.map((r) => r.x));
  const minY = Math.min(...all.map((r) => r.y));
  const maxX = Math.max(...all.map((r) => r.x + r.m.resolutionX));
  const maxY = Math.max(...all.map((r) => r.y + r.m.resolutionY));
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

  const sel = monitors.find((m) => m.id === selected);
  const selPos = sel ? draft[sel.id]! : null;
  const selDelta = sel && selPos ? { x: selPos.x - system[sel.id]!.x, y: selPos.y - system[sel.id]!.y } : null;

  const setSelectedCoord = (axis: "x" | "y", value: number) => {
    if (!sel || !selPos || Number.isNaN(value)) return;
    const dx = axis === "x" ? value - selPos.x : 0;
    const dy = axis === "y" ? value - selPos.y : 0;
    setDraft((d) => moveMonitor(monitors, d, sel.id, dx, dy));
    setMessage(null);
  };

  return (
    <div>
      <div className="section-title-row">
        <div className="section-title">Virtual Layout</div>
        {!editing && (
          <button
            className="btn btn-secondary btn-small btn-icon"
            onClick={startEditing}
            title="Fine-tune the Windows display arrangement"
            disabled={monitors.length < 2}
          >
            <PencilIcon /> Edit
          </button>
        )}
      </div>
      <div
        ref={mapRef}
        className={`layout-map-container${editing ? " layout-map-editing" : ""}`}
        tabIndex={editing ? 0 : -1}
        onKeyDown={onKeyDown}
      >
        <svg
          width={svgW}
          height={svgH}
          viewBox={`0 0 ${svgW} ${svgH}`}
          className="layout-map-svg"
        >
          {editing &&
            monitors.map((m) => {
              const p = system[m.id]!;
              const q = draft[m.id]!;
              if (p.x === q.x && p.y === q.y) return null;
              return (
                <rect
                  key={`ghost-${m.id}`}
                  x={(p.x - minX) * scale + padding}
                  y={(p.y - minY) * scale + padding}
                  width={m.resolutionX * scale}
                  height={m.resolutionY * scale}
                  rx={4}
                  fill="none"
                  stroke="var(--text-muted)"
                  strokeWidth={1}
                  strokeDasharray="4 3"
                />
              );
            })}
          {monitors.map((m, i) => {
            const p = positions[m.id]!;
            const x = (p.x - minX) * scale + padding;
            const y = (p.y - minY) * scale + padding;
            const w = m.resolutionX * scale;
            const h = m.resolutionY * scale;
            const color = COLORS[i % COLORS.length]!;
            const isSel = editing && m.id === selected;

            const label = m.friendlyName || `Display ${m.id + 1}`;
            const fontSize = Math.min(w * 0.14, h * 0.28, 16);
            const numSize = Math.min(w * 0.3, h * 0.5, 36);

            return (
              <g
                key={m.id}
                onMouseDown={
                  editing
                    ? (e) => {
                        e.preventDefault();
                        setSelected(m.id);
                        mapRef.current?.focus();
                      }
                    : undefined
                }
                style={editing ? { cursor: "pointer" } : undefined}
              >
                <rect
                  x={x}
                  y={y}
                  width={w}
                  height={h}
                  rx={4}
                  fill={color.bg}
                  stroke={isSel ? "#fff" : color.border}
                  strokeWidth={isSel ? 2.5 : 1.5}
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
                  {editing ? `${p.x}, ${p.y}` : `${m.resolutionX}x${m.resolutionY}`}
                </text>
              </g>
            );
          })}
        </svg>
      </div>

      {editing && (
        <div className="layout-edit-panel">
          <div className="layout-edit-hint">
            Click a display (or press 1–9), then use the arrow keys to move it 1 px —
            Shift for 10 px, Ctrl for 100 px. Nothing changes in Windows until you
            click Apply.
          </div>

          {sel && selPos && (
            <div className="layout-edit-selected">
              <strong>
                Display {sel.id + 1}
                {sel.isPrimary ? " (primary)" : ""}
              </strong>
              {sel.isPrimary ? (
                <span className="layout-edit-muted">
                  The primary display stays at 0, 0 — moving it shifts the others.
                </span>
              ) : (
                <>
                  <label>
                    X
                    <input
                      type="number"
                      value={selPos.x}
                      onChange={(e) => setSelectedCoord("x", parseInt(e.target.value, 10))}
                    />
                  </label>
                  <label>
                    Y
                    <input
                      type="number"
                      value={selPos.y}
                      onChange={(e) => setSelectedCoord("y", parseInt(e.target.value, 10))}
                    />
                  </label>
                </>
              )}
              {selDelta && (selDelta.x !== 0 || selDelta.y !== 0) && (
                <span className="layout-edit-muted">
                  ({selDelta.x >= 0 ? "+" : ""}
                  {selDelta.x}, {selDelta.y >= 0 ? "+" : ""}
                  {selDelta.y} from system)
                </span>
              )}
            </div>
          )}
          {sel && (
            <ul className="layout-edit-offsets">
              {edgeOffsets(monitors, draft, sel.id).map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          )}

          {validation && <div className="layout-edit-error">{validation}</div>}
          {message && (
            <div className={message.error ? "layout-edit-error" : "layout-edit-ok"}>
              {message.text}
            </div>
          )}

          <div className="actions">
            <button
              className="btn btn-accent btn-small"
              onClick={() => apply(draft)}
              disabled={!dirty || !!validation || applying}
            >
              {applying ? "Applying…" : "Apply"}
            </button>
            {dirty && (
              <button
                className="btn btn-secondary btn-small"
                onClick={() => {
                  setDraft(system);
                  setMessage(null);
                }}
                disabled={applying}
              >
                Reset to system settings
              </button>
            )}
            {undo && !dirty && (
              <button
                className="btn btn-secondary btn-small"
                onClick={() => apply(undo, true)}
                disabled={applying}
              >
                Undo apply
              </button>
            )}
            <button
              className="btn btn-secondary btn-small"
              onClick={stopEditing}
              disabled={applying}
            >
              {dirty ? "Cancel" : "Done"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
