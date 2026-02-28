interface Props {
  monitorCount: number;
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

export default function StatusBar({ monitorCount, loading, error, onRefresh }: Props) {
  return (
    <div className="status-bar">
      <span
        className={`status-dot ${
          error
            ? "status-dot-error"
            : loading
              ? "status-dot-loading"
              : "status-dot-success"
        }`}
      />
      <span className="status-text">
        {loading
          ? "Discovering monitors..."
          : error
            ? error
            : `${monitorCount} monitor${monitorCount !== 1 ? "s" : ""} detected`}
      </span>
      <button
        className="btn btn-secondary btn-small status-refresh"
        onClick={onRefresh}
        disabled={loading}
      >
        Refresh
      </button>
    </div>
  );
}
