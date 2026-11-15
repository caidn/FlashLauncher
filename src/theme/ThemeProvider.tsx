import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { DEFAULT_ACCENT, accentById, readableOn } from "./accents";

export type ThemeMode = "dark" | "light";
export type MotionPref = "system" | "reduced";

interface ThemeState {
  accent: string;
  mode: ThemeMode;
  motion: MotionPref;
  setAccent: (id: string) => void;
  setMode: (mode: ThemeMode) => void;
  setMotion: (m: MotionPref) => void;
}

const ThemeContext = createContext<ThemeState | null>(null);
const KEY = "flashlauncher.theme";

// The settings table in the library DB is the source of truth. localStorage
// is a cache so the first frame already has the right colors.
function load(): Partial<Pick<ThemeState, "accent" | "mode" | "motion">> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const saved = load();
  const [accent, setAccent] = useState(saved.accent ?? DEFAULT_ACCENT);
  const [mode, setMode] = useState<ThemeMode>(saved.mode ?? "dark");
  const [motion, setMotion] = useState<MotionPref>(saved.motion ?? "system");
  const hydrated = useRef(false);

  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        const t = s.theme ? JSON.parse(s.theme) : null;
        if (t?.accent) setAccent(t.accent);
        if (t?.mode) setMode(t.mode);
        if (t?.motion) setMotion(t.motion);
      })
      .catch(() => {})
      .finally(() => (hydrated.current = true));
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    const a = accentById(accent);
    root.dataset.theme = mode;
    root.dataset.motion = motion;
    root.style.setProperty("--accent", a.hex);
    root.style.setProperty("--accent-fg", readableOn(a.hex));
    const json = JSON.stringify({ accent, mode, motion });
    try {
      localStorage.setItem(KEY, json);
    } catch {
      /* storage can be unavailable; the DB copy still holds */
    }
    if (hydrated.current) api.setSetting("theme", json).catch(() => {});
  }, [accent, mode, motion]);

  return (
    <ThemeContext.Provider value={{ accent, mode, motion, setAccent, setMode, setMotion }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): ThemeState {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used inside ThemeProvider");
  return ctx;
}

/** True when animations should be skipped (system setting or user override). */
export function useReducedMotion(): boolean {
  const { motion } = useTheme();
  const [system, setSystem] = useState(
    () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false,
  );
  useEffect(() => {
    const mq = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!mq) return;
    const on = () => setSystem(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return motion === "reduced" || system;
}
