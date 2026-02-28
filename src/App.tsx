import { useCallback, useEffect, useState } from "react";
import MonitorList from "./components/MonitorList";
import CalibrationPanel from "./components/CalibrationPanel";
import StatusBar from "./components/StatusBar";
import { useTauriCommands } from "./hooks/useTauriCommands";
import type { CalibrationStatus, Monitor } from "./types";

export default function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [calibrationStatus, setCalibrationStatus] =
    useState<CalibrationStatus>("idle");

  const { discoverMonitors } = useTauriCommands();

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await discoverMonitors();
      setMonitors(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [discoverMonitors]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleCalibrate = () => {
    setCalibrationStatus("in_progress");
    // TODO: invoke calibration flow
    setTimeout(() => setCalibrationStatus("idle"), 500);
  };

  return (
    <div className="app">
      <div className="app-header">
        <div>
          <h1>Spanright Calibrate</h1>
          <div className="subtitle">
            Multi-monitor layout calibration for Spanright
          </div>
        </div>
        <button
          className="btn btn-secondary btn-small"
          onClick={refresh}
          disabled={loading}
        >
          Refresh
        </button>
      </div>

      <div>
        <div className="section-title">Detected Monitors</div>
        <MonitorList monitors={monitors} />
      </div>

      <CalibrationPanel
        monitorCount={monitors.length}
        status={calibrationStatus}
        onCalibrate={handleCalibrate}
      />

      <div style={{ marginTop: "auto" }}>
        <StatusBar
          monitorCount={monitors.length}
          loading={loading}
          error={error}
        />
      </div>
    </div>
  );
}
