interface Props {
  onCopyJson: () => void;
  onSaveFile: () => void;
  onOpenSpanright: () => void;
  spanrightReady: boolean;
  copied: boolean;
}

export default function ExportPanel({
  onCopyJson,
  onSaveFile,
  onOpenSpanright,
  spanrightReady,
  copied,
}: Props) {
  return (
    <div>
      <div className="section-title">Export</div>
      <div className="actions">
        <button
          className="btn btn-accent"
          onClick={onOpenSpanright}
          disabled={!spanrightReady}
        >
          Open in Spanright
        </button>
        <button className="btn btn-secondary" onClick={onCopyJson}>
          {copied ? "Copied!" : "Copy JSON"}
        </button>
        <button className="btn btn-secondary" onClick={onSaveFile}>
          Export JSON
        </button>
      </div>
    </div>
  );
}
