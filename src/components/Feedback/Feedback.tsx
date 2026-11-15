import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import s from "./Feedback.module.css";

/** The Windows 8 indeterminate progress: five dots racing across. */
export function ProgressDots({ label }: { label?: string }) {
  return (
    <div className={s.dots} role="progressbar" aria-label={label ?? "Working"}>
      {[0, 1, 2, 3, 4].map((i) => (
        <span key={i} style={{ ["--d" as string]: i }} />
      ))}
    </div>
  );
}

/** Determinate bar: flat accent fill on a faint track. */
export function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className={s.bar} role="progressbar" aria-valuemin={0} aria-valuemax={max} aria-valuenow={value}>
      <span style={{ width: `${pct}%` }} />
    </div>
  );
}

// ---------------------------------------------------------------- toasts

interface Toast {
  id: number;
  title: string;
  body?: string;
}

const ToastCtx = createContext<(t: Omit<Toast, "id">) => void>(() => {});

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const push = useCallback((t: Omit<Toast, "id">) => {
    const id = Date.now() + Math.random();
    setToasts((prev) => [...prev.slice(-2), { ...t, id }]);
    window.setTimeout(() => setToasts((prev) => prev.filter((x) => x.id !== id)), 5000);
  }, []);

  return (
    <ToastCtx.Provider value={push}>
      {children}
      {createPortal(
        <div className={s.toasts} aria-live="polite">
          {toasts.map((t) => (
            <button
              key={t.id}
              className={s.toast}
              onClick={() => setToasts((prev) => prev.filter((x) => x.id !== t.id))}
            >
              <span className={s.toastTitle}>{t.title}</span>
              {t.body && <span className={s.toastBody}>{t.body}</span>}
            </button>
          ))}
        </div>,
        document.getElementById("layer") ?? document.body,
      )}
    </ToastCtx.Provider>
  );
}

export function useToast() {
  return useContext(ToastCtx);
}
