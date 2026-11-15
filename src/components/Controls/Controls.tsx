import type { ButtonHTMLAttributes, ReactNode } from "react";
import { useTilt } from "../../lib/useTilt";
import s from "./Controls.module.css";

export function Button({
  variant = "outline",
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "outline" | "accent" }) {
  const tilt = useTilt<HTMLButtonElement>(6);
  return (
    <button {...rest} {...tilt} className={s.button} data-variant={variant} data-nav>
      {children}
    </button>
  );
}

/** Windows 8 toggle: rectangular track, square thumb, label on the left. */
export function Toggle({
  label,
  checked,
  onChange,
  hint,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  hint?: string;
}) {
  return (
    <label className={s.toggleRow}>
      <span className={s.toggleText}>
        <span>{label}</span>
        {hint && <span className={s.hint}>{hint}</span>}
      </span>
      <span className={s.toggleRight}>
        <span className={s.toggleState}>{checked ? "on" : "off"}</span>
        <button
          role="switch"
          aria-checked={checked}
          className={s.toggle}
          data-on={checked}
          data-nav
          onClick={() => onChange(!checked)}
        >
          <span className={s.thumb} />
        </button>
      </span>
    </label>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: ReactNode; title?: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className={s.segmented} role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={value === o.value}
          title={o.title}
          data-active={value === o.value}
          data-nav
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** Square Metro checkbox. */
export function Checkbox({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      className={s.checkbox}
      data-on={checked}
      disabled={disabled}
      data-nav
      onClick={() => onChange(!checked)}
    />
  );
}

/** Flat text box: filled at rest, accent border while editing. */
export function TextField({
  value,
  onChange,
  label,
  autoFocus,
  onEnter,
  invalid,
}: {
  value: string;
  onChange: (v: string) => void;
  label: string;
  autoFocus?: boolean;
  onEnter?: () => void;
  invalid?: boolean;
}) {
  return (
    <input
      className={s.text}
      value={value}
      aria-label={label}
      aria-invalid={invalid}
      data-autofocus={autoFocus ? "" : undefined}
      spellCheck={false}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && onEnter) onEnter();
      }}
    />
  );
}

/** Metro slider: flat track, accent fill, tall rectangular thumb. */
export function Slider({
  value,
  onChange,
  label,
  min = 0,
  max = 100,
}: {
  value: number;
  onChange: (v: number) => void;
  label: string;
  min?: number;
  max?: number;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <input
      type="range"
      className={s.slider}
      min={min}
      max={max}
      value={value}
      aria-label={label}
      style={{ ["--pct" as string]: `${pct}%` }}
      onChange={(e) => onChange(Number(e.target.value))}
    />
  );
}
