import { useMemo, useRef, useState } from "react";
import type { CalibrationResult, Monitor } from "../types";
import { applyMonitorPositions } from "../hooks/useTauriCommands";
import { computePhysicalPlacements } from "../physicalLayout";
import {
  constrainPosition,
  edgeOffsets,
  suggestFromPhysical,
  type Pos,
  type SnapGuide,
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
  /** Calibration results, used to suggest a layout matching the desk. */
  results: CalibrationResult[];
}

/** Screen pixels within which a dragged display snaps to an alignment. */
const SNAP_SCREEN_PX = 8;

interface DragState {
  id: number;
  pointerId: number;
  startClientX: number;
  startClientY: number;
  startPos: Pos;
  /** Draft when the drag started; only the dragged display moves from it. */
  base: Positions;
  /** Map transform frozen for the drag so the map doesn't rescale under the cursor. */
  view: { minX: number; minY: number; scale: number; svgW: number; svgH: number };
  moved: boolean;
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

export default function MonitorLayoutMap({ monitors, onApplied, results }: Props) {
  const system = useMemo(() => positionsOf(monitors), [monitors]);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Positions>(system);
  const [selected, setSelected] = useState<number | null>(null);
  const [applying, setApplying] = useState(false);
  const [message, setMessage] = useState<{ text: string; error?: boolean } | null>(null);
  /** System positions from before the last Apply, so it can be undone. */
  const [undo, setUndo] = useState<Positions | null>(null);
  const mapRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const dragRef = useRef<DragState | null>(null);
  const [dragView, setDragView] = useState<{
    positions: Positions;
    guide: SnapGuide | null;
  } | null>(null);

  const physical = useMemo(
    () => (results.length ? computePhysicalPlacements(monitors, results) : null),
    [monitors, results]
  );
  const suggestion = useMemo(
    () => (physical ? suggestFromPhysical(monitors, physical, results) : null),
    [monitors, physical, results]
  );

  // Whenever Windows' layout changes (refresh, calibration, apply), start the
  // draft over from it so we never edit stale positions. Done during render
  // (not in an effect) so a draft missing a new monitor is never drawn.
  // A re-read that changes nothing (e.g. after editing a monitor's size) keeps
  // the draft.
  const [prevSystem, setPrevSystem] = useState(system);
  if (prevSystem !== system) {
    setPrevSystem(system);
    if (!samePositions(prevSystem, system)) {
      setDraft(system);
      return null;
    }
  }

  if (monitors.length === 0) return null;

  const dragging = editing && dragView != null && dragRef.current != null;
  const positions = editing ? (dragging ? dragView!.positions : draft) : system;
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

  /** Arrow keys: slide along the attached edge, never leaving a gap. */
  const move = (dx: number, dy: number) => {
    if (selected == null) return;
    const cur = draft[selected]!;
    const res = constrainPosition(monitors, draft, selected, { x: cur.x + dx, y: cur.y + dy });
    if (!res) return;
    setDraft(moveMonitor(monitors, draft, selected, res.pos.x - cur.x, res.pos.y - cur.y));
    setMessage(null);
  };

  const applySuggestion = () => {
    if (!suggestion) return;
    setDraft(suggestion);
    const err = validateLayout(monitors, suggestion);
    setMessage(
      err
        ? { text: `Suggested layout needs adjusting: ${err}`, error: true }
        : {
            text: samePositions(suggestion, system)
              ? "Your layout already matches the calibrated desk."
              : "Suggested layout loaded from your calibration. Review it, then click Apply.",
          }
    );
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
          ? { text: "Applied, but Windows adjusted the layout. Check the positions below." }
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

  const fitView = {
    minX,
    minY,
    scale,
    svgW: totalW * scale + padding * 2,
    svgH: totalH * scale + padding * 2,
  };
  const view = dragging ? dragRef.current!.view : fitView;
  const { svgW, svgH } = view;
  const sx = (x: number) => (x - view.minX) * view.scale + padding;
  const sy = (y: number) => (y - view.minY) * view.scale + padding;

  const primaryMon = monitors.find((m) => m.isPrimary);
  const origin = primaryMon ? positions[primaryMon.id]! : { x: 0, y: 0 };

  const onPointerDown = (e: React.PointerEvent, id: number) => {
    if (!editing || e.button !== 0) return;
    e.preventDefault();
    setSelected(id);
    mapRef.current?.focus();
    svgRef.current?.setPointerCapture(e.pointerId);
    dragRef.current = {
      id,
      pointerId: e.pointerId,
      startClientX: e.clientX,
      startClientY: e.clientY,
      startPos: draft[id]!,
      base: draft,
      view: fitView,
      moved: false,
    };
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const d = dragRef.current;
    if (!d || e.pointerId !== d.pointerId) return;
    const ddx = e.clientX - d.startClientX;
    const ddy = e.clientY - d.startClientY;
    if (!d.moved && Math.hypot(ddx, ddy) < 3) return;
    d.moved = true;
    const desired = {
      x: Math.round(d.startPos.x + ddx / d.view.scale),
      y: Math.round(d.startPos.y + ddy / d.view.scale),
    };
    // Hold Alt to drag without snapping.
    const snap = e.altKey ? 0 : SNAP_SCREEN_PX / d.view.scale;
    const res = constrainPosition(monitors, d.base, d.id, desired, snap);
    if (!res) return;
    setDragView({ positions: { ...d.base, [d.id]: res.pos }, guide: res.guide });
  };

  const endDrag = (e: React.PointerEvent) => {
    const d = dragRef.current;
    if (!d || e.pointerId !== d.pointerId) return;
    dragRef.current = null;
    const final = dragView?.positions[d.id];
    setDragView(null);
    if (d.moved && final) {
      setDraft(moveMonitor(monitors, d.base, d.id, final.x - d.startPos.x, final.y - d.startPos.y));
      setMessage(null);
    }
  };

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
          ref={svgRef}
          style={{ overflow: "visible" }}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
        >
          {editing &&
            monitors.map((m) => {
              const p = system[m.id]!;
              const q = draft[m.id]!;
              if (p.x === q.x && p.y === q.y) return null;
              return (
                <rect
                  key={`ghost-${m.id}`}
                  x={sx(p.x)}
                  y={sy(p.y)}
                  width={m.resolutionX * view.scale}
                  height={m.resolutionY * view.scale}
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
            const x = sx(p.x);
            const y = sy(p.y);
            const w = m.resolutionX * view.scale;
            const h = m.resolutionY * view.scale;
            const color = COLORS[i % COLORS.length]!;
            const isSel = editing && m.id === selected;

            const label = m.friendlyName || `Display ${m.id + 1}`;
            const fontSize = Math.min(w * 0.14, h * 0.28, 16);
            const numSize = Math.min(w * 0.3, h * 0.5, 36);

            return (
              <g
                key={m.id}
                onPointerDown={(e) => onPointerDown(e, m.id)}
                style={
                  editing
                    ? { cursor: dragging && m.id === dragRef.current?.id ? "grabbing" : "grab" }
                    : undefined
                }
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
                  {editing
                    ? `${p.x - origin.x}, ${p.y - origin.y}`
                    : `${m.resolutionX}x${m.resolutionY}`}
                </text>
              </g>
            );
          })}
          {dragging && dragView!.guide && (
            dragView!.guide.axis === "x" ? (
              <line
                x1={sx(dragView!.guide.at)}
                x2={sx(dragView!.guide.at)}
                y1={0}
                y2={svgH}
                className="layout-snap-guide"
              />
            ) : (
              <line
                x1={0}
                x2={svgW}
                y1={sy(dragView!.guide.at)}
                y2={sy(dragView!.guide.at)}
                className="layout-snap-guide"
              />
            )
          )}
        </svg>
      </div>

      {editing && (
        <div className="layout-edit-panel">
          <div className="layout-edit-hint">
            Drag a display to slide it along (and around) its neighbours. It snaps to
            aligned edges; hold Alt to place it freely. For pixel-exact nudges, select a
            display (click or press 1–9) and use the arrow keys: 1 px, Shift 10 px, Ctrl
            100 px. Nothing changes in Windows until you click Apply.
          </div>

          {sel && selPos && (
            <div className="layout-edit-selected">
              <strong>
                Display {sel.id + 1}
                {sel.isPrimary ? " (primary)" : ""}
              </strong>
              {sel.isPrimary ? (
                <span className="layout-edit-muted">
                  The primary display stays at 0, 0, so moving it shifts the others.
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
              {edgeOffsets(monitors, positions, sel.id).map((t) => (
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
            <button
              className="btn btn-secondary btn-small"
              onClick={applySuggestion}
              disabled={!suggestion || applying}
              title={
                suggestion
                  ? "Arrange displays so the cursor crosses each edge where the screens physically meet"
                  : "Run a calibration first to get a suggestion from your physical layout"
              }
            >
              Match physical layout
            </button>
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
