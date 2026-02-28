import { useCallback, useEffect, useState } from "react";
import MonitorLayoutMap from "./components/MonitorLayoutMap";
import PhysicalLayoutMap from "./components/PhysicalLayoutMap";
import MonitorList from "./components/MonitorList";
import CalibrationPanel from "./components/CalibrationPanel";
import ExportPanel from "./components/ExportPanel";
import StatusBar from "./components/StatusBar";
import {
  discoverMonitors,
  startCalibration,
  exportCalibrationJson,
  saveCalibrationFile,
  openUrl,
} from "./hooks/useTauriCommands";
import { buildSpanrightUrl } from "./spanrightUrl";
import type { CalibrationResult, CalibrationStatus, Monitor } from "./types";

export default function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [calibrationStatus, setCalibrationStatus] =
    useState<CalibrationStatus>("idle");
  const [calibrationResults, setCalibrationResults] = useState<
    CalibrationResult[]
  >([]);

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
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleCalibrate = async () => {
    setCalibrationStatus("in_progress");
    setError(null);
    try {
      const results = await startCalibration();
      setCalibrationResults(results);
      setCalibrationStatus("complete");
      const freshMonitors = await discoverMonitors();
      setMonitors(freshMonitors);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("cancelled")) {
        setCalibrationStatus("idle");
      } else {
        setError(msg);
        setCalibrationStatus("error");
      }
    }
  };

  const [copyFeedback, setCopyFeedback] = useState(false);

  const handleCopyJson = async () => {
    try {
      const json = await exportCalibrationJson(calibrationResults);
      await navigator.clipboard.writeText(json);
      setError(null);
      setCopyFeedback(true);
      setTimeout(() => setCopyFeedback(false), 2000);
    } catch (e) {
      setError(
        `Export failed: ${e instanceof Error ? e.message : String(e)}`
      );
    }
  };

  const handleSaveFile = async () => {
    try {
      const result = await saveCalibrationFile(calibrationResults);
      if (result !== "cancelled") {
        setError(null);
      }
    } catch (e) {
      setError(
        `Save failed: ${e instanceof Error ? e.message : String(e)}`
      );
    }
  };

  const handleOpenSpanright = async () => {
    const url = buildSpanrightUrl(monitors, calibrationResults);
    if (!url) {
      setError("Could not build Spanright URL — ensure monitors have diagonal sizes set");
      return;
    }
    try {
      await openUrl(url);
    } catch (e) {
      setError(`Failed to open Spanright: ${e instanceof Error ? e.message : String(e)}`);
    }
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

      <StatusBar
        monitorCount={monitors.length}
        loading={loading}
        error={error}
      />

      <MonitorLayoutMap monitors={monitors} />

      <div>
        <div className="section-title">Detected Monitors</div>
        <MonitorList monitors={monitors} onRefresh={refresh} />
      </div>

      <CalibrationPanel
        monitorCount={monitors.length}
        status={calibrationStatus}
        results={calibrationResults}
        monitors={monitors}
        onCalibrate={handleCalibrate}
      />

      {calibrationStatus === "complete" && calibrationResults.length > 0 && (
        <>
          <PhysicalLayoutMap
            monitors={monitors}
            results={calibrationResults}
          />
          <ExportPanel
            onCopyJson={handleCopyJson}
            onSaveFile={handleSaveFile}
            onOpenSpanright={handleOpenSpanright}
            spanrightReady={monitors.some((m) => m.ppi != null)}
            copied={copyFeedback}
          />
        </>
      )}

    </div>
  );
}
