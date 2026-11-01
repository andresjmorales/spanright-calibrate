import { describe, expect, it } from "vitest";
import type { Monitor } from "./types";
import { computePhysicalPlacements } from "./physicalLayout";
import {
  constrainPosition,
  edgeOffsets,
  moveMonitor,
  positionsOf,
  suggestFromPhysical,
  validateLayout,
  type Positions,
} from "./layoutEdit";

function mon(
  id: number,
  x: number,
  y: number,
  w: number,
  h: number,
  opts: Partial<Monitor> = {}
): Monitor {
  return {
    id,
    deviceName: `\\\\.\\DISPLAY${id + 1}`,
    friendlyName: `Display ${id + 1}`,
    monitorName: "",
    adapterName: "",
    monitorDeviceId: "",
    isPrimary: id === 0,
    resolutionX: w,
    resolutionY: h,
    positionX: x,
    positionY: y,
    orientation: 0,
    physicalWidthMm: null,
    physicalHeightMm: null,
    physicalWidthIn: null,
    physicalHeightIn: null,
    diagonalIn: null,
    ppi: null,
    sizeSource: "edid",
    ...opts,
  };
}

/** Primary 1440p in the middle, 1080p to the right, portrait 1080p to the left. */
function desk() {
  return [
    mon(0, 0, 0, 2560, 1440),
    mon(1, 2560, 180, 1920, 1080),
    mon(2, -1080, -200, 1080, 1920),
  ];
}

const at = (p: Positions, id: number) => p[id]!;

describe("validateLayout", () => {
  it("accepts touching monitors with an offset", () => {
    const m = desk();
    expect(validateLayout(m, positionsOf(m))).toBeNull();
  });

  it("rejects overlaps", () => {
    const m = desk();
    const p = { ...positionsOf(m), 1: { x: 2559, y: 180 } };
    expect(validateLayout(m, p)).toMatch(/overlap/);
  });

  it("rejects gaps", () => {
    const m = desk();
    const p = { ...positionsOf(m), 1: { x: 2561, y: 180 } };
    expect(validateLayout(m, p)).toMatch(/Display 2 must share an edge/);
  });

  it("rejects corner-only contact", () => {
    const m = desk();
    const p = { ...positionsOf(m), 1: { x: 2560, y: 1440 } };
    expect(validateLayout(m, p)).not.toBeNull();
  });

  it("accepts a monitor connected only through another non-primary one", () => {
    const m = [
      mon(0, 0, 0, 1920, 1080),
      mon(1, 1920, 0, 1920, 1080),
      mon(2, 1920, 1080, 1920, 1080),
    ];
    expect(validateLayout(m, positionsOf(m))).toBeNull();
  });
});

describe("moveMonitor", () => {
  it("moves a non-primary monitor", () => {
    const m = desk();
    const p = moveMonitor(m, positionsOf(m), 1, 3, -2);
    expect(at(p, 1)).toEqual({ x: 2563, y: 178 });
    expect(at(p, 0)).toEqual({ x: 0, y: 0 });
  });

  it("keeps the primary at the origin by shifting the others", () => {
    const m = desk();
    const p = moveMonitor(m, positionsOf(m), 0, 0, 10);
    expect(at(p, 0)).toEqual({ x: 0, y: 0 });
    expect(at(p, 1)).toEqual({ x: 2560, y: 170 });
    expect(at(p, 2)).toEqual({ x: -1080, y: -210 });
  });
});

describe("edgeOffsets", () => {
  it("describes the vertical offset of side-by-side neighbours", () => {
    const m = desk();
    expect(edgeOffsets(m, positionsOf(m), 1)).toEqual([
      "Top edge 180 px below Display 1",
    ]);
    expect(edgeOffsets(m, positionsOf(m), 2)).toEqual([
      "Top edge 200 px above Display 1",
    ]);
  });

  it("describes the horizontal offset of stacked neighbours", () => {
    const m = [mon(0, 0, 0, 2560, 1440), mon(1, 320, -1080, 1920, 1080)];
    expect(edgeOffsets(m, positionsOf(m), 1)).toEqual([
      "Left edge 320 px right of Display 1",
    ]);
  });
});

describe("constrainPosition", () => {
  it("slides along the attached edge", () => {
    const m = desk();
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2560, y: 100 });
    expect(r?.pos).toEqual({ x: 2560, y: 100 });
  });

  it("never leaves a gap when pulled away", () => {
    const m = desk();
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2700, y: 180 });
    expect(r?.pos).toEqual({ x: 2560, y: 180 });
  });

  it("never overlaps when pushed in", () => {
    const m = desk();
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2400, y: 180 });
    expect(r?.pos).toEqual({ x: 2560, y: 180 });
  });

  it("stops at the end of an edge for a 1 px nudge (no corner-only contact)", () => {
    const m = desk();
    const p = { ...positionsOf(m), 1: { x: 2560, y: 1439 } };
    // Arrow-key down from the last valid row: stays put rather than jumping
    // onto the bottom edge or touching only at the corner.
    const r = constrainPosition(m, p, 1, { x: 2560, y: 1440 });
    expect(r?.pos).toEqual({ x: 2560, y: 1439 });
  });

  it("wraps under the bottom corner when dragged well past it", () => {
    const m = desk();
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2560, y: 1600 });
    expect(r?.pos).toEqual({ x: 2559, y: 1440 });
  });

  it("wraps around a corner onto the next edge", () => {
    const m = desk();
    // Dragged above and to the left of the primary's top-right corner.
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2000, y: -1200 });
    expect(r?.pos).toEqual({ x: 2000, y: -1080 });
    expect(validateLayout(m, { ...positionsOf(m), 1: r!.pos })).toBeNull();
  });

  it("does not slide through another monitor", () => {
    const m = desk();
    // Above the primary but over where the portrait monitor sits: it has to
    // stop clear of the portrait monitor.
    const r = constrainPosition(m, positionsOf(m), 1, { x: -1500, y: -1080 });
    const p = { ...positionsOf(m), 1: r!.pos };
    expect(validateLayout(m, p)).toBeNull();
  });

  it("refuses moves that would disconnect the rest of the layout", () => {
    // Display 3 hangs off display 2, so display 2 can't leave display 1's
    // right edge without stranding it... unless it stays connected.
    const m = [
      mon(0, 0, 0, 1920, 1080),
      mon(1, 1920, 0, 1920, 1080),
      mon(2, 3840, 0, 1920, 1080),
    ];
    const r = constrainPosition(m, positionsOf(m), 1, { x: 0, y: -1080 });
    const p = { ...positionsOf(m), 1: r!.pos };
    expect(validateLayout(m, p)).toBeNull();
  });

  it("snaps to top alignment within the snap distance, with a guide", () => {
    const m = desk();
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2560, y: 6 }, 10);
    expect(r?.pos).toEqual({ x: 2560, y: 0 });
    expect(r?.guide).toEqual({ axis: "y", at: 0 });
  });

  it("snaps to bottom and centre alignment", () => {
    const m = desk();
    const bottom = constrainPosition(m, positionsOf(m), 1, { x: 2560, y: 355 }, 10);
    expect(bottom?.pos.y).toBe(360);
    expect(bottom?.guide).toEqual({ axis: "y", at: 1440 });
    const centre = constrainPosition(m, positionsOf(m), 1, { x: 2560, y: 185 }, 10);
    expect(centre?.pos.y).toBe(180);
    expect(centre?.guide).toEqual({ axis: "y", at: 720 });
  });

  it("does not snap without a snap distance", () => {
    const m = desk();
    const r = constrainPosition(m, positionsOf(m), 1, { x: 2560, y: 6 });
    expect(r?.pos).toEqual({ x: 2560, y: 6 });
    expect(r?.guide).toBeNull();
  });
});

describe("suggestFromPhysical", () => {
  // 27" 1440p in the middle, 24" 1080p on the right, both vertically centred.
  const ppi27 = Math.hypot(2560, 1440) / 27;
  const ppi24 = Math.hypot(1920, 1080) / 24;

  function centredDesk() {
    const m = [
      mon(0, 0, 0, 2560, 1440, { ppi: ppi27 }),
      mon(1, 2560, 0, 1920, 1080, { ppi: ppi24 }),
    ];
    // Midpoint lines sit at the same physical height: the centre of each panel.
    const results = [
      {
        monitorId: 1,
        boundTo: 0,
        bindHorizontal: true,
        scale: ppi24 / ppi27,
        relativeX: 0,
        relativeY: 0,
        gap: 0,
        alignOffsetUnbound: 540,
        alignOffsetBound: 720,
      },
    ];
    return { m, results };
  }

  it("lines the panels up at the middle of the shared physical edge", () => {
    const { m, results } = centredDesk();
    const placements = computePhysicalPlacements(m, results)!;
    const s = suggestFromPhysical(m, placements, results)!;
    expect(at(s, 0)).toEqual({ x: 0, y: 0 });
    // Row 720 of the 27" and row 540 of the 24" are at the same height.
    expect(at(s, 1)).toEqual({ x: 2560, y: 180 });
    expect(validateLayout(m, s)).toBeNull();
  });

  it("gives top alignment for same-PPI panels whose tops are level", () => {
    const m = [
      mon(0, 0, 0, 1920, 1080, { ppi: 92 }),
      mon(1, 1920, 300, 1920, 1080, { ppi: 92 }),
    ];
    const results = [
      {
        monitorId: 1,
        boundTo: 0,
        bindHorizontal: true,
        scale: 1,
        relativeX: 0,
        relativeY: 0,
        gap: 0,
        alignOffsetUnbound: 400,
        alignOffsetBound: 400,
      },
    ];
    const s = suggestFromPhysical(m, computePhysicalPlacements(m, results)!, results)!;
    expect(at(s, 1)).toEqual({ x: 1920, y: 0 });
  });

  it("chains three monitors and puts each on its physical side", () => {
    const m = [
      mon(0, 0, 0, 2560, 1440, { ppi: 109 }),
      mon(1, 2560, 0, 1920, 1080, { ppi: 92 }),
      mon(2, -1920, 0, 1920, 1080, { ppi: 92 }),
    ];
    const pair = (id: number, unbound: number, bound: number) => ({
      monitorId: id,
      boundTo: 0,
      bindHorizontal: true,
      scale: 92 / 109,
      relativeX: 0,
      relativeY: 0,
      gap: 0,
      alignOffsetUnbound: unbound,
      alignOffsetBound: bound,
    });
    const results = [pair(1, 540, 720), pair(2, 300, 300)];
    const s = suggestFromPhysical(m, computePhysicalPlacements(m, results)!, results)!;
    expect(at(s, 1).x).toBe(2560);
    expect(at(s, 2).x).toBe(-1920);
    expect(validateLayout(m, s)).toBeNull();
  });

  it("normalizes so the primary is at the origin", () => {
    const m = [
      mon(0, 1920, 0, 1920, 1080, { ppi: 92, isPrimary: false }),
      mon(1, 0, 0, 1920, 1080, { ppi: 92, isPrimary: true }),
    ];
    const results = [
      {
        monitorId: 1,
        boundTo: 0,
        bindHorizontal: true,
        scale: 1,
        relativeX: 0,
        relativeY: 0,
        gap: 0,
        alignOffsetUnbound: 500,
        alignOffsetBound: 400,
      },
    ];
    const s = suggestFromPhysical(m, computePhysicalPlacements(m, results)!, results)!;
    expect(at(s, 1)).toEqual({ x: 0, y: 0 });
    expect(at(s, 0)).toEqual({ x: 1920, y: 100 });
  });

  it("returns null when a monitor has no physical placement", () => {
    const m = desk();
    expect(suggestFromPhysical(m, [], [])).toBeNull();
  });
});
