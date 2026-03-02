interface Props {
  onCopyJson: () => void;
  onSaveFile: () => void;
  onOpenSpanright: () => void;
  spanrightReady: boolean;
  copied: boolean;
  includeVirtualLayout: boolean;
  onToggleVirtualLayout: () => void;
}

export default function ExportPanel({
  onCopyJson,
  onSaveFile,
  onOpenSpanright,
  spanrightReady,
  copied,
  includeVirtualLayout,
  onToggleVirtualLayout,
}: Props) {
  return (
    <div>
      <div className="section-title">Export</div>
      <label className="virtual-layout-toggle">
        <input
          type="checkbox"
          checked={includeVirtualLayout}
          onChange={onToggleVirtualLayout}
        />
        <span>Include virtual layout</span>
      </label>
      <div className="actions">
        <button
          className="btn btn-accent"
          onClick={onOpenSpanright}
          disabled={!spanrightReady}
        >
          Open in Spanright
        </button>
        <button className="btn btn-secondary btn-fixed-copy" onClick={onCopyJson}>
          {copied ? "Copied!" : "Copy JSON"}
        </button>
        <button className="btn btn-secondary" onClick={onSaveFile}>
          Export JSON
        </button>
      </div>
    </div>
  );
}
