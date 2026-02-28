import { useState, useEffect } from "react";
import type { Monitor } from "../types";
import {
  setMonitorDiagonal,
  getMonitorInfo,
  type MonitorDetailInfo,
} from "../hooks/useTauriCommands";

interface Props {
  monitors: Monitor[];
  onRefresh: () => void;
}

function formatPpi(monitor: Monitor): string {
  if (monitor.ppi != null) {
    return `${Math.round(monitor.ppi)} PPI`;
  }
  return "";
}

function DiagonalField({
  monitor,
  onRefresh,
}: {
  monitor: Monitor;
  onRefresh: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(
    monitor.diagonalIn != null ? monitor.diagonalIn.toFixed(1) : ""
  );
  const [saving, setSaving] = useState(false);

  const save = async () => {
    const num = parseFloat(value);
    if (!num || num <= 0 || num > 200) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      await setMonitorDiagonal(monitor.id, num);
      onRefresh();
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
      setEditing(false);
    }
  };

  if (editing) {
    return (
      <span className="diagonal-edit">
        <input
          className="diagonal-input"
          type="number"
          step="0.1"
          min="1"
          max="200"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") save();
            if (e.key === "Escape") setEditing(false);
          }}
          onBlur={save}
          autoFocus
          disabled={saving}
        />
        "
      </span>
    );
  }

  return (
    <span
      className="diagonal-display"
      onClick={() => {
        setValue(
          monitor.diagonalIn != null ? monitor.diagonalIn.toFixed(1) : ""
        );
        setEditing(true);
      }}
      title="Click to edit diagonal size"
    >
      {monitor.diagonalIn != null
        ? `${monitor.diagonalIn.toFixed(1)}"`
        : "? size"}
    </span>
  );
}

const MANUFACTURER_NAMES: Record<string, string> = {
  ACI: "ASUS",
  ACR: "Acer",
  AOC: "AOC",
  AUO: "AU Optronics",
  BNQ: "BenQ",
  CMN: "Chimei Innolux",
  DEL: "Dell",
  EIZ: "EIZO",
  GSM: "LG Electronics",
  HPN: "HP",
  HWP: "HP",
  IVM: "Iiyama",
  LEN: "Lenovo",
  LGD: "LG Display",
  MEI: "Panasonic",
  MSI: "MSI",
  NEC: "NEC",
  PHL: "Philips",
  SAM: "Samsung",
  SEC: "Samsung (panel)",
  SHP: "Sharp",
  SNY: "Sony",
  VSC: "ViewSonic",
};

function resolveManufacturer(code: string): string {
  const name = MANUFACTURER_NAMES[code];
  return name ? `${name} (${code})` : code;
}

function InfoRow({ label, value }: { label: string; value: string | null }) {
  if (!value) return null;
  return (
    <div className="info-row">
      <span className="info-label">{label}</span>
      <span className="info-value">{value}</span>
    </div>
  );
}

function MonitorInfoPopup({
  monitor,
  onClose,
}: {
  monitor: Monitor;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<MonitorDetailInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);
    getMonitorInfo(monitor.id)
      .then((data) => {
        setDetail(data);
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  }, [monitor.id]);

  const info = detail?.edid;

  return (
    <div className="info-overlay" onClick={onClose}>
      <div className="info-popup" onClick={(e) => e.stopPropagation()}>
        <div className="info-popup-header">
          <span className="info-popup-title">
            {monitor.friendlyName || monitor.monitorName || `Display ${monitor.id + 1}`}
          </span>
          <button className="info-close" onClick={onClose}>
            ✕
          </button>
        </div>
        {loading && <div className="info-loading">Loading...</div>}
        {error && <div className="info-error">{error}</div>}
        {!loading && !error && (
          <div className="info-body">
            {info ? (
              <>
                <InfoRow label="Monitor Name" value={info.monitorName} />
                <InfoRow label="Manufacturer" value={resolveManufacturer(info.manufacturer)} />
                <InfoRow
                  label="Product Code"
                  value={`0x${info.productCode.toString(16).toUpperCase().padStart(4, "0")}`}
                />
                <InfoRow
                  label="Serial (EDID)"
                  value={info.monitorSerial || (info.serialNumber ? String(info.serialNumber) : null)}
                />
                <InfoRow label="EDID Version" value={info.edidVersion} />
                <InfoRow
                  label="Manufactured"
                  value={
                    info.manufactureYear
                      ? `Week ${info.manufactureWeek}, ${info.manufactureYear}`
                      : null
                  }
                />
                <InfoRow
                  label="Panel Size (EDID)"
                  value={
                    info.widthMm && info.heightMm
                      ? `${info.widthMm} × ${info.heightMm} mm`
                      : null
                  }
                />
                <InfoRow
                  label="Native Resolution"
                  value={
                    info.nativeResolution
                      ? `${info.nativeResolution[0]} × ${info.nativeResolution[1]}`
                      : null
                  }
                />
                <InfoRow label="Color Format" value={info.displayType} />
                <InfoRow
                  label="Bit Depth"
                  value={info.bitDepth ? `${info.bitDepth}-bit` : null}
                />
                <InfoRow
                  label="Gamma"
                  value={info.gamma ? info.gamma.toFixed(2) : null}
                />
                <InfoRow
                  label="Refresh Rate Range"
                  value={
                    info.minVRateHz && info.maxVRateHz
                      ? `${info.minVRateHz} – ${info.maxVRateHz} Hz`
                      : null
                  }
                />
                <InfoRow
                  label="H Freq Range"
                  value={
                    info.minHRateKhz && info.maxHRateKhz
                      ? `${info.minHRateKhz} – ${info.maxHRateKhz} kHz`
                      : null
                  }
                />
                <InfoRow
                  label="Max Pixel Clock"
                  value={
                    info.maxPixelClockMhz
                      ? `${info.maxPixelClockMhz} MHz`
                      : null
                  }
                />
                <InfoRow
                  label="DPMS"
                  value={
                    [
                      info.dpmsStandby && "Standby",
                      info.dpmsSuspend && "Suspend",
                      info.dpmsOff && "Off",
                    ]
                      .filter(Boolean)
                      .join(", ") || "Not supported"
                  }
                />
              </>
            ) : (
              <div className="info-empty" style={{ padding: "8px 0" }}>
                No EDID data available.
              </div>
            )}
            <div className="info-section-title">System Info</div>
            <InfoRow label="Adapter" value={monitor.adapterName} />
            <InfoRow
              label="Connection"
              value={detail?.connectionType ?? null}
            />
            <InfoRow
              label="Refresh Rate"
              value={
                detail?.refreshRateHz
                  ? `${detail.refreshRateHz} Hz`
                  : null
              }
            />
            <InfoRow
              label="Resolution"
              value={`${monitor.resolutionX} × ${monitor.resolutionY}`}
            />
            <InfoRow
              label="Position"
              value={`(${monitor.positionX}, ${monitor.positionY})`}
            />
            <InfoRow
              label="PPI"
              value={monitor.ppi ? `${Math.round(monitor.ppi)}` : null}
            />
            <InfoRow
              label="Device ID"
              value={monitor.monitorDeviceId || null}
            />
          </div>
        )}
      </div>
    </div>
  );
}

export default function MonitorList({ monitors, onRefresh }: Props) {
  const [infoMonitor, setInfoMonitor] = useState<Monitor | null>(null);

  if (monitors.length === 0) {
    return <div className="empty-state">No monitors detected.</div>;
  }

  return (
    <>
      <div className="monitor-list">
        {monitors.map((m) => {
          const displayName =
            m.friendlyName || m.monitorName || `Display ${m.id + 1}`;
          const ppi = formatPpi(m);

          return (
            <div className="monitor-card" key={m.id}>
              <div className="monitor-icon">🖥</div>
              <div className="monitor-info">
                <div className="monitor-name">
                  {displayName}
                  {m.isPrimary && (
                    <span className="badge badge-primary">Primary</span>
                  )}
                  {m.sizeSource === "edid" ? (
                    <span className="badge badge-edid">EDID</span>
                  ) : m.sizeSource === "manual" ? (
                    <span className="badge badge-edid">Manual</span>
                  ) : m.sizeSource === "guessed" ? (
                    <span className="badge badge-no-edid">Estimated</span>
                  ) : (
                    <span className="badge badge-no-edid">No size</span>
                  )}
                  <button
                    className="info-btn"
                    onClick={() => setInfoMonitor(m)}
                    title="View detailed monitor info"
                  >
                    ℹ
                  </button>
                </div>
                <div className="monitor-details">
                  <DiagonalField monitor={m} onRefresh={onRefresh} />
                  <span>
                    {m.resolutionX}×{m.resolutionY}
                  </span>
                  {ppi && <span>{ppi}</span>}
                  <span>
                    ({m.positionX}, {m.positionY})
                  </span>
                </div>
              </div>
            </div>
          );
        })}
      </div>
      {infoMonitor && (
        <MonitorInfoPopup
          monitor={infoMonitor}
          onClose={() => setInfoMonitor(null)}
        />
      )}
    </>
  );
}
