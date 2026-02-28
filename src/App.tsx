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
import AboutDialog from "./components/AboutDialog";
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
  const [showAbout, setShowAbout] = useState(false);

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
            Multi-monitor layout calibration for{" "}
            <a
              href="#"
              className="subtle-link"
              onClick={(e) => {
                e.preventDefault();
                openUrl("https://spanright.com");
              }}
            >
              Spanright
            </a>
          </div>
        </div>
        <div className="header-actions">
          <button
            className="btn btn-secondary btn-small"
            onClick={() => setShowAbout(true)}
          >
            About
          </button>
          <a
            href="#"
            className="github-link"
            onClick={(e) => {
              e.preventDefault();
              openUrl("https://github.com/andresjmorales/spanright-calibrate");
            }}
            title="View on GitHub"
          >
            <svg viewBox="0 0 16 16" width="20" height="20" fill="currentColor">
              <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
            </svg>
          </a>
        </div>
      </div>

      <StatusBar
        monitorCount={monitors.length}
        loading={loading}
        error={error}
        onRefresh={refresh}
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

      {showAbout && (
        <AboutDialog
          onClose={() => setShowAbout(false)}
          onOpenUrl={openUrl}
        />
      )}
    </div>
  );
}
