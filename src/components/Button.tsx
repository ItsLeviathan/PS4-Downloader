import type { ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "./Icon";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "md" | "sm";
  icon?: IconName;
  busy?: boolean;
}

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  busy = false,
  className = "",
  children,
  disabled,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`btn btn-${variant} btn-${size} ${className}`}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      {...rest}
    >
      {busy ? (
        <span className="spinner" aria-hidden="true" />
      ) : (
        icon && <Icon name={icon} size={size === "sm" ? 15 : 17} />
      )}
      {children && <span>{children}</span>}
    </button>
  );
}
