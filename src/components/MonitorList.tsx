import { useState } from "react";
import type { Monitor } from "../types";
import { setMonitorDiagonal } from "../hooks/useTauriCommands";

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
      <span className="edit-hint">✎</span>
    </span>
  );
}

export default function MonitorList({ monitors, onRefresh }: Props) {
  if (monitors.length === 0) {
    return <div className="empty-state">No monitors detected.</div>;
  }

  return (
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
              </div>
              <div className="monitor-details">
                <span>
                  {m.resolutionX}×{m.resolutionY}
                </span>
                <DiagonalField monitor={m} onRefresh={onRefresh} />
                {ppi && <span>{ppi}</span>}
                <span>
                  at ({m.positionX}, {m.positionY})
                </span>
              </div>
              <div className="monitor-adapter">{m.adapterName}</div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
