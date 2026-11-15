import type { ReactNode } from "react";
import { Icon, type IconName } from "../Icon/Icon";
import s from "./AppBar.module.css";

// Windows 8 app bar: slides up from the bottom edge when something is
// selected. Context text on the left, commands on the right.

export function AppBar({ open, children, left }: { open: boolean; children: ReactNode; left?: ReactNode }) {
  return (
    <div className={s.bar} data-open={open} inert={!open} role="toolbar">
      <div className={s.left}>{left}</div>
      <div className={s.commands}>{children}</div>
    </div>
  );
}

export function AppBarCommand({
  icon,
  label,
  onClick,
  disabled,
}: {
  icon: IconName;
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button className={s.command} onClick={onClick} disabled={disabled} data-nav>
      <span className={s.glyph}>
        <Icon name={icon} size={20} />
      </span>
      <span>{label}</span>
    </button>
  );
}
