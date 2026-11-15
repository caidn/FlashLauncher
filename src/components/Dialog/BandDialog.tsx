import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import s from "./BandDialog.module.css";

// Windows 8 message dialog: a full-width band across the middle of a dimmed
// screen, content aligned to the app's left edge, buttons bottom right.

let openCount = 0;

function setAppInert(on: boolean) {
  const root = document.getElementById("root");
  if (!root) return;
  openCount += on ? 1 : -1;
  root.inert = openCount > 0;
}

export function BandDialog({
  title,
  children,
  actions,
  onCancel,
  wide = false,
  tall = false,
}: {
  title: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
  /** Esc and clicking the dimmed area call this. Omit for dialogs that can't be dismissed. */
  onCancel?: () => void;
  wide?: boolean;
  /** Lets the band grow taller for long lists. */
  tall?: boolean;
}) {
  const band = useRef<HTMLDivElement>(null);
  const cancel = useRef(onCancel);
  cancel.current = onCancel;

  useEffect(() => {
    setAppInert(true);
    const prev = document.activeElement as HTMLElement | null;
    const el = band.current;
    const first = el?.querySelector<HTMLElement>("[data-autofocus]") ?? el?.querySelector<HTMLElement>("button, input");
    first?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && cancel.current) {
        e.stopPropagation();
        cancel.current();
      }
      if (e.key === "Tab" && el) {
        const items = Array.from(el.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled)"));
        if (!items.length) return;
        const i = items.indexOf(document.activeElement as HTMLElement);
        const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : i === items.length - 1 ? 0 : i + 1;
        items[next].focus();
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      setAppInert(false);
      prev?.focus?.();
    };
  }, []);

  return createPortal(
    <div className={s.scrim} onMouseDown={(e) => e.target === e.currentTarget && onCancel?.()}>
      <div className={s.band} ref={band} role="dialog" aria-modal="true" data-tall={tall}>
        <div className={s.inner} data-wide={wide}>
          <h2 className={s.title}>{title}</h2>
          {children && <div className={s.body}>{children}</div>}
          {actions && <div className={s.actions}>{actions}</div>}
        </div>
      </div>
    </div>,
    document.getElementById("layer") ?? document.body,
  );
}
