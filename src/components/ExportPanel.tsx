interface Props {
  onExportJson: () => void;
}

export default function ExportPanel({ onExportJson }: Props) {
  return (
    <div>
      <div className="section-title">Export</div>
      <div className="actions">
        <button className="btn btn-secondary" onClick={onExportJson}>
          Copy JSON to Clipboard
        </button>
      </div>
    </div>
  );
}
