import type { Monitor } from "../types";

interface Props {
  monitors: Monitor[];
}

function formatDiagonal(monitor: Monitor): string {
  if (monitor.diagonalIn != null) {
    return `${monitor.diagonalIn.toFixed(1)}"`;
  }
  return "unknown size";
}

function formatPpi(monitor: Monitor): string {
  if (monitor.ppi != null) {
    return `${Math.round(monitor.ppi)} PPI`;
  }
  return "";
}

export default function MonitorList({ monitors }: Props) {
  if (monitors.length === 0) {
    return <div className="empty-state">No monitors detected.</div>;
  }

  return (
    <div className="monitor-list">
      {monitors.map((m) => {
        const displayName =
          m.friendlyName || m.monitorName || `Display ${m.id + 1}`;
        const hasEdid = m.physicalWidthMm != null;
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
                {hasEdid ? (
                  <span className="badge badge-edid">EDID</span>
                ) : (
                  <span className="badge badge-no-edid">No EDID</span>
                )}
              </div>
              <div className="monitor-details">
                <span>
                  {m.resolutionX}×{m.resolutionY}
                </span>
                <span>{formatDiagonal(m)}</span>
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
