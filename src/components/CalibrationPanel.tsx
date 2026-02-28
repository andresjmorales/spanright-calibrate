import type { CalibrationStatus } from "../types";

interface Props {
  monitorCount: number;
  status: CalibrationStatus;
  onCalibrate: () => void;
}

export default function CalibrationPanel({
  monitorCount,
  status,
  onCalibrate,
}: Props) {
  const canCalibrate = monitorCount >= 2 && status !== "in_progress";

  const statusText: Record<CalibrationStatus, string> = {
    idle: "Not calibrated",
    in_progress: "Calibration in progress...",
    complete: "Calibration complete",
    error: "Calibration failed",
  };

  return (
    <div className="calibration-panel">
      <div className="actions">
        <button
          className="btn btn-accent"
          disabled={!canCalibrate}
          onClick={onCalibrate}
        >
          Calibrate
        </button>
        <span className="status-bar" style={{ flex: 1 }}>
          <span
            className={`status-dot ${
              status === "idle"
                ? "status-dot-idle"
                : status === "in_progress"
                  ? "status-dot-loading"
                  : status === "complete"
                    ? "status-dot-success"
                    : "status-dot-error"
            }`}
          />
          {statusText[status]}
        </span>
      </div>
      {monitorCount < 2 && (
        <div className="error-message">
          At least 2 monitors are required for calibration.
        </div>
      )}
    </div>
  );
}
