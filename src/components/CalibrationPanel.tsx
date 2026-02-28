import type { CalibrationResult, CalibrationStatus, Monitor } from "../types";

interface Props {
  monitorCount: number;
  status: CalibrationStatus;
  results: CalibrationResult[];
  monitors: Monitor[];
  onCalibrate: () => void;
}

export default function CalibrationPanel({
  monitorCount,
  status,
  results,
  monitors,
  onCalibrate,
}: Props) {
  const canCalibrate = monitorCount >= 2 && status !== "in_progress";

  const statusText: Record<CalibrationStatus, string> = {
    idle: "Not calibrated",
    in_progress: "Calibration in progress...",
    complete: "Calibration complete",
    error: "Calibration failed",
  };

  const getMonitorName = (id: number) => {
    const m = monitors.find((mon) => mon.id === id);
    if (!m) return `Display ${id + 1}`;
    return m.friendlyName || m.monitorName || `Display ${id + 1}`;
  };

  return (
    <div className="calibration-panel">
      <div className="actions">
        <button
          className="btn btn-accent"
          disabled={!canCalibrate}
          onClick={onCalibrate}
        >
          {status === "complete" ? "Recalibrate" : "Calibrate"}
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

      {results.length > 0 && (
        <div>
          <div className="section-title">Calibration Results</div>
          <div className="monitor-list">
            {(() => {
              const calibratedIds = new Set(results.map((r) => r.monitorId));
              const referenceMonitors = monitors.filter(
                (m) => !calibratedIds.has(m.id)
              );
              return referenceMonitors.map((m) => (
                <div className="monitor-card" key={`ref-${m.id}`}>
                  <div className="monitor-icon">📍</div>
                  <div className="monitor-info">
                    <div className="monitor-name">
                      {getMonitorName(m.id)}
                      <span className="badge badge-primary">Reference</span>
                    </div>
                    <div className="monitor-details">
                      <span>scale 1.000</span>
                      <span>
                        {m.resolutionX}x{m.resolutionY}
                        {m.isPrimary ? " (primary)" : ""}
                      </span>
                    </div>
                  </div>
                </div>
              ));
            })()}
            {results.map((r) => (
              <div className="monitor-card" key={r.monitorId}>
                <div className="monitor-icon">📐</div>
                <div className="monitor-info">
                  <div className="monitor-name">
                    {getMonitorName(r.monitorId)}
                  </div>
                  <div className="monitor-details">
                    <span>scale {r.scale.toFixed(3)}</span>
                    <span>gap {r.gap}px</span>
                    <span>
                      bound to {getMonitorName(r.boundTo)}
                      {r.bindHorizontal ? " (horizontal)" : " (vertical)"}
                    </span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
