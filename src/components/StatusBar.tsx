interface Props {
  monitorCount: number;
  loading: boolean;
  error: string | null;
}

export default function StatusBar({ monitorCount, loading, error }: Props) {
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
      {loading
        ? "Discovering monitors..."
        : error
          ? error
          : `${monitorCount} monitor${monitorCount !== 1 ? "s" : ""} detected`}
    </div>
  );
}
