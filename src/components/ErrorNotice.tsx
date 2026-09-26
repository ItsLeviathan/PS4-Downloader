import { useState } from "react";
import type { AppError } from "../types";
import { Icon } from "./Icon";

interface ErrorNoticeProps {
  error: AppError;
  tone?: "error" | "warning";
}

/** A plain-language error with the technical cause behind "View details". */
export function ErrorNotice({ error, tone = "error" }: ErrorNoticeProps) {
  const [open, setOpen] = useState(false);
  return (
    <div className={`notice notice-${tone}`} role="alert">
      <Icon name="alert" size={17} className="notice-icon" />
      <div className="notice-body">
        <p className="notice-message">{error.message}</p>
        {error.details && (
          <>
            <button type="button" className="link-button" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
              {open ? "Hide details" : "View details"}
            </button>
            {open && <pre className="notice-details">{error.details}</pre>}
          </>
        )}
      </div>
    </div>
  );
}
