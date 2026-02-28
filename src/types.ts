export interface Monitor {
  id: number;
  deviceName: string;
  friendlyName: string;
  monitorName: string;
  adapterName: string;
  monitorDeviceId: string;
  isPrimary: boolean;
  resolutionX: number;
  resolutionY: number;
  positionX: number;
  positionY: number;
  orientation: number;
  physicalWidthMm: number | null;
  physicalHeightMm: number | null;
  physicalWidthIn: number | null;
  physicalHeightIn: number | null;
  diagonalIn: number | null;
  ppi: number | null;
  sizeSource: "edid" | "guessed" | "manual" | "none";
}

export interface CalibrationResult {
  monitorId: number;
  scale: number;
  relativeX: number;
  relativeY: number;
  gap: number;
  boundTo: number;
  bindHorizontal: boolean;
  alignOffsetUnbound: number;
  alignOffsetBound: number;
}

export type CalibrationStatus = "idle" | "in_progress" | "complete" | "error";

export interface AppState {
  monitors: Monitor[];
  calibrationResults: CalibrationResult[];
  calibrationStatus: CalibrationStatus;
  error: string | null;
  loading: boolean;
}
