import { useEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "./Button";

interface ConfirmDialogProps {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  /** Optional opt-in checkbox, e.g. "Also delete the file". Unchecked by default. */
  option?: string;
  onConfirm: (optionChecked: boolean) => void;
  onClose: () => void;
}

export function ConfirmDialog({ title, children, confirmLabel, danger, option, onConfirm, onClose }: ConfirmDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    ref.current?.showModal();
  }, []);

  return (
    <dialog ref={ref} className="dialog" onClose={onClose}>
      <h2 className="dialog-title">{title}</h2>
      <div className="dialog-body">{children}</div>
      {option && (
        <label className="checkbox">
          <input type="checkbox" checked={checked} onChange={(e) => setChecked(e.target.checked)} />
          <span>{option}</span>
        </label>
      )}
      <div className="dialog-actions">
        <Button variant="ghost" onClick={onClose} autoFocus>
          Go back
        </Button>
        <Button
          variant={danger ? "danger" : "primary"}
          onClick={() => {
            onConfirm(checked);
            onClose();
          }}
        >
          {confirmLabel}
        </Button>
      </div>
    </dialog>
  );
}
