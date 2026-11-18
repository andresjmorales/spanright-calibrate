/**
 * Linux calibration overlay. The Rust side opens one full-screen window per
 * monitor (label "overlay-<monitor index>") and each renders this component.
 * Every window draws the same scene in virtual-desktop pixels, translated to its
 * own monitor, so lines continue across monitors exactly like the Windows GDI
 * overlay (src-tauri/src/calibration/overlay/win.rs), which this mirrors.
 * Line changes are broadcast so all windows stay in sync; Enter/Esc report the
 * result back to Rust.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

type RGB = [number, number, number];
interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}
interface OverlayView {
  token: number;
  step: "scale" | "gap";
  m1Idx: number;
  m2Idx: number;
  monitors: Rect[];
  labels: string[];
  bindHorizontal: boolean;
  segments: [number, number, number, number];
  midM1: number;
  midM2: number;
  color1: RGB;
  color2: RGB;
}
interface Lines {
  token: number;
  segments: [number, number, number, number];
  gap: number;
  lastInteracted: number | null;
}
interface LinesMessage extends Lines {
  from: string;
}

const STATE_EVENT = "overlay://state";
const VIEW_EVENT = "overlay://view";

const css = (c: RGB) => `rgb(${c[0]},${c[1]},${c[2]})`;

export default function Overlay({ label, monitorIdx }: { label: string; monitorIdx: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [view, setView] = useState<OverlayView | null>(null);
  const [lines, setLines] = useState<Lines | null>(null);
  const [hovering, setHovering] = useState(false);
  const drag = useRef<{ idx: number | null; start: number; startVal: number } | null>(null);
  const finished = useRef<number | null>(null);

  const applyView = useCallback((v: OverlayView | null) => {
    if (!v) return;
    setView(v);
    setLines((l) =>
      l && l.token === v.token ? l : { token: v.token, segments: v.segments, gap: 0, lastInteracted: null }
    );
  }, []);

  // Page setup, the current step, and updates from the other overlay windows.
  useEffect(() => {
    document.body.style.margin = "0";
    document.body.style.overflow = "hidden";
    document.body.style.background = "rgb(10,12,18)";
    const unlisten = [
      listen<OverlayView>(VIEW_EVENT, (e) => applyView(e.payload)),
      listen<LinesMessage>(STATE_EVENT, (e) => {
        const { from, ...l } = e.payload;
        if (from === label) return;
        setLines((cur) => (cur && cur.token !== l.token ? cur : l));
      }),
    ];
    invoke<OverlayView | null>("overlay_view").then(applyView);
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, [label, applyView]);

  const update = useCallback(
    (next: Lines) => {
      setLines(next);
      emit(STATE_EVENT, { ...next, from: label } satisfies LinesMessage);
    },
    [label]
  );

  const rect = view?.monitors[monitorIdx];

  // Window (CSS) pixels to virtual-desktop pixels. The canvas is backed by the
  // monitor's real pixel count, which may differ from CSS pixels when scaled.
  const toVirtual = (clientX: number, clientY: number): [number, number] => {
    if (!rect) return [0, 0];
    return [rect.x + (clientX * rect.w) / window.innerWidth, rect.y + (clientY * rect.h) / window.innerHeight];
  };

  // --- Drawing ---------------------------------------------------------------
  useEffect(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    if (!view || !lines || !rect) {
      ctx.fillStyle = "rgb(10,12,18)";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      return;
    }
    canvas.width = rect.w;
    canvas.height = rect.h;
    ctx.setTransform(1, 0, 0, 1, -rect.x, -rect.y);
    drawScene(ctx, view, lines);
  }, [view, lines, rect]);

  // --- Input -----------------------------------------------------------------
  const finish = useCallback(
    (cancelled: boolean) => {
      if (!view || !lines || finished.current === view.token) return;
      finished.current = view.token;
      invoke("overlay_finish", {
        token: view.token,
        cancelled,
        segments: lines.segments,
        gap: lines.gap,
      });
    },
    [view, lines]
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!view || !lines) return;
      if (e.key === "Enter") return finish(false);
      if (e.key === "Escape") return finish(true);
      const delta = { ArrowUp: -1, ArrowLeft: -1, ArrowDown: 1, ArrowRight: 1 }[e.key];
      if (delta === undefined) return;
      e.preventDefault();
      if (view.step === "scale") {
        const idx = lines.lastInteracted;
        if (idx === null) return;
        const segments = [...lines.segments] as Lines["segments"];
        segments[idx] = segments[idx]! + delta;
        update({ ...lines, segments });
      } else {
        update({ ...lines, gap: lines.gap + delta });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, lines, finish, update]);

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!view || !lines || e.button !== 0) return;
    const [mx, my] = toVirtual(e.clientX, e.clientY);
    if (view.step === "scale") {
      const idx = hitTestScale(view, lines, mx, my);
      if (idx === null) return;
      drag.current = { idx, start: view.bindHorizontal ? my : mx, startVal: lines.segments[idx]! };
      update({ ...lines, lastInteracted: idx });
    } else {
      drag.current = { idx: null, start: view.bindHorizontal ? mx : my, startVal: lines.gap };
    }
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!view || !lines) return;
    const [mx, my] = toVirtual(e.clientX, e.clientY);
    const d = drag.current;
    if (!d) {
      setHovering(view.step === "scale" ? hitTestScale(view, lines, mx, my) !== null : hitTestGap(view, mx, my));
      return;
    }
    if (view.step === "scale" && d.idx !== null) {
      const segments = [...lines.segments] as Lines["segments"];
      segments[d.idx] = Math.round(d.startVal + (view.bindHorizontal ? my : mx) - d.start);
      if (segments[d.idx] !== lines.segments[d.idx]) update({ ...lines, segments });
    } else if (view.step === "gap") {
      const gap = Math.round(d.startVal + (view.bindHorizontal ? mx : my) - d.start);
      if (gap !== lines.gap) update({ ...lines, gap });
    }
  };

  const endDrag = () => {
    drag.current = null;
  };

  return (
    <canvas
      ref={canvasRef}
      style={{
        position: "fixed",
        inset: 0,
        width: "100vw",
        height: "100vh",
        display: "block",
        cursor: hovering || drag.current ? "pointer" : "default",
      }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
    />
  );
}

function hitTestScale(v: OverlayView, l: Lines, mx: number, my: number): number | null {
  const m1 = v.monitors[v.m1Idx]!;
  const m2 = v.monitors[v.m2Idx]!;
  const threshold = 20;
  const owners = [m1, m2, m1, m2];
  for (let i = 0; i < 4; i++) {
    const m = owners[i]!;
    const seg = l.segments[i]!;
    if (v.bindHorizontal) {
      if (mx >= m.x && mx <= m.x + m.w && Math.abs(my - seg) <= threshold) return i;
    } else if (my >= m.y && my <= m.y + m.h && Math.abs(mx - seg) <= threshold) {
      return i;
    }
  }
  return null;
}

function hitTestGap(v: OverlayView, mx: number, my: number): boolean {
  const m1 = v.monitors[v.m1Idx]!;
  const m2 = v.monitors[v.m2Idx]!;
  const threshold = 150;
  if (v.bindHorizontal) {
    const boundary = m1.x < m2.x ? m1.x + m1.w : m2.x + m2.w;
    return Math.abs(mx - boundary) <= threshold;
  }
  const boundary = m1.y < m2.y ? m1.y + m1.h : m2.y + m2.h;
  return Math.abs(my - boundary) <= threshold;
}

function drawScene(ctx: CanvasRenderingContext2D, v: OverlayView, l: Lines) {
  const m1 = v.monitors[v.m1Idx]!;
  const m2 = v.monitors[v.m2Idx]!;

  // Background: dark everywhere, slightly lighter on the pair being calibrated.
  ctx.fillStyle = "rgb(10,12,18)";
  for (const m of v.monitors) ctx.fillRect(m.x, m.y, m.w, m.h);
  ctx.fillStyle = "rgb(22,25,32)";
  for (const m of [m1, m2]) ctx.fillRect(m.x, m.y, m.w, m.h);

  ctx.strokeStyle = "rgb(60,65,80)";
  ctx.lineWidth = 1;
  for (const m of [m1, m2]) ctx.strokeRect(m.x + 0.5, m.y + 0.5, m.w - 1, m.h - 1);

  v.monitors.forEach((m, i) => drawMonitorLabel(ctx, m, v.labels[i] ?? ""));

  const c1 = css(v.color1);
  const c2 = css(v.color2);

  if (v.step === "scale") {
    const t = 6;
    const s = l.segments;
    if (v.bindHorizontal) {
      fill(ctx, m1.x, s[0] - t / 2, m1.w, t, c1);
      fill(ctx, m2.x, s[1] - t / 2, m2.w, t, c1);
      fill(ctx, m1.x, s[2] - t / 2, m1.w, t, c2);
      fill(ctx, m2.x, s[3] - t / 2, m2.w, t, c2);
    } else {
      fill(ctx, s[0] - t / 2, m1.y, t, m1.h, c1);
      fill(ctx, s[1] - t / 2, m2.y, t, m2.h, c1);
      fill(ctx, s[2] - t / 2, m1.y, t, m1.h, c2);
      fill(ctx, s[3] - t / 2, m2.y, t, m2.h, c2);
    }
    const text = v.bindHorizontal
      ? "Drag each colored line so it sits at the same physical height on both displays.  Keep both lines far apart for best accuracy.  |  Arrow keys: ±1px  |  Enter: confirm  |  Esc: cancel"
      : "Drag each colored line so it sits at the same physical left-right position on both displays.  Keep both lines far apart for best accuracy.  |  Arrow keys: ±1px  |  Enter: confirm  |  Esc: cancel";
    drawText(ctx, m1.x + 20, m1.y + m1.h - 60, text);
    drawText(ctx, m2.x + 20, m2.y + m2.h - 60, text);
    return;
  }

  const gap = l.gap;
  const pen = 4;
  const inset = pen + 2;
  if (v.bindHorizontal) {
    const [leftMid, rightMid, lm, rm] = m1.x < m2.x ? [v.midM1, v.midM2, m1, m2] : [v.midM2, v.midM1, m2, m1];
    const bx = lm.x + lm.w;
    const bxRight = rm.x;
    const arm = Math.max(Math.floor((Math.min(lm.w, rm.w) * 2) / 5), 150);
    // All lines stay at 45°; the gap translates the right monitor's lines vertically.
    line(ctx, bx - arm, leftMid - arm, bx - inset, leftMid - inset, c1, pen);
    line(ctx, bx - arm, leftMid + arm, bx - inset, leftMid + inset, c2, pen);
    line(ctx, bxRight + inset, rightMid + gap + inset, bxRight + arm, rightMid + gap + arm, c1, pen);
    line(ctx, bxRight + inset, rightMid - gap - inset, bxRight + arm, rightMid - gap - arm, c2, pen);
  } else {
    const [topMid, bottomMid, tm, bm] = m1.y < m2.y ? [v.midM1, v.midM2, m1, m2] : [v.midM2, v.midM1, m2, m1];
    const by = tm.y + tm.h;
    const byBottom = bm.y;
    const arm = Math.max(Math.floor((Math.min(tm.h, bm.h) * 2) / 5), 150);
    line(ctx, topMid - arm, by - arm, topMid - inset, by - inset, c1, pen);
    line(ctx, topMid + arm, by - arm, topMid + inset, by - inset, c2, pen);
    line(ctx, bottomMid + gap + inset, byBottom + inset, bottomMid + gap + arm, byBottom + arm, c1, pen);
    line(ctx, bottomMid - gap - inset, byBottom + inset, bottomMid - gap - arm, byBottom + arm, c2, pen);
  }
  const text = `Gap: ${gap}px  |  Drag or arrow keys to adjust so both diagonal lines are aligned  |  Enter: confirm  |  Esc: cancel`;
  drawText(ctx, m1.x + 20, m1.y + m1.h - 40, text);
  drawText(ctx, m2.x + 20, m2.y + m2.h - 40, text);
}

function fill(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, color: string) {
  ctx.fillStyle = color;
  ctx.fillRect(x, y, w, h);
}

function line(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, color: string, width: number) {
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.lineCap = "round";
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.stroke();
}

function drawText(ctx: CanvasRenderingContext2D, x: number, y: number, text: string) {
  ctx.fillStyle = "rgb(160,165,175)";
  ctx.font = "16px system-ui, sans-serif";
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  ctx.fillText(text, x, y);
}

function drawMonitorLabel(ctx: CanvasRenderingContext2D, m: Rect, label: string) {
  if (!label) return;
  const fontHeight = Math.min(Math.max(Math.floor(m.h / 35), 20), 70);
  ctx.font = `${fontHeight}px system-ui, sans-serif`;
  ctx.fillStyle = "rgb(80,85,105)";
  ctx.textBaseline = "top";
  ctx.textAlign = "center";
  const lines = label.split("\n");
  const spacing = fontHeight + 6;
  const startY = m.y + (m.h - lines.length * spacing) / 2;
  lines.forEach((text, i) => ctx.fillText(text, m.x + m.w / 2, startY + i * spacing));
}
