// Reusable confirmation dialog — React-based, no native window.confirm.
// Used for destructive actions (delete collection, request, environment, clear history).

interface ConfirmDialogProps {
  message: string;
  title?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export default function ConfirmDialog({
  message,
  title = "Confirm",
  confirmLabel = "Delete",
  cancelLabel = "Cancel",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <div
      className="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div className="modal-panel" style={{ maxWidth: 400 }}>
        <div className="modal-panel__header">
          <span className="modal-panel__title">{title}</span>
        </div>
        <div className="modal-panel__body" style={{ padding: "20px" }}>
          <p style={{ margin: 0, color: "var(--color-text)", fontSize: 14, lineHeight: 1.5 }}>
            {message}
          </p>
        </div>
        <div
          className="modal-panel__footer"
          style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "0 20px 20px" }}
        >
          <button className="btn btn--secondary" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button className="btn btn--danger" onClick={onConfirm} autoFocus>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
