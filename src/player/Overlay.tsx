import { useEffect, useRef } from "react";
import { Button, Segmented, Slider, Toggle } from "../components/Controls/Controls";
import type { Game, GamePatch, Quality, ScaleMode } from "../data/types";
import s from "./Player.module.css";

interface Props {
  game: Game;
  fullscreen: boolean;
  volume: number;
  muted: boolean;
  pendingRestart: boolean;
  onResume: () => void;
  onScreenshot: () => void;
  onFullscreen: () => void;
  onRestart: () => void;
  onQuit: () => void;
  onVolume: (v: number) => void;
  onMute: (m: boolean) => void;
  onPatch: (p: GamePatch, needsRestart: boolean) => void;
}

const FRAME_RATES = ["auto", "12", "24", "30", "60"] as const;

// The in-game menu. Left: what to do. Right: how this game plays.
export function Overlay(p: Props) {
  const { game } = p;
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => first.current?.focus(), []);

  const letterboxes = [
    { value: "#000000", label: "black" },
    { value: "#1c1c1c", label: "dark" },
    ...(game.background && !["#000000", "#1c1c1c"].includes(game.background)
      ? [{ value: game.background, label: "game color" }]
      : []),
  ];
  const fr = game.frameRateOverride ? String(game.frameRateOverride) : "auto";

  return (
    <div className={s.overlay} role="dialog" aria-modal="true" aria-label="Game menu">
      <div className={s.overlayInner}>
        <section className={s.menuCol}>
          <span className={s.coverLabel}>paused</span>
          <h1 className={s.menuTitle}>{game.title}</h1>
          <nav className={s.menu}>
            <MenuItem first={first} label="resume" keys="esc" onClick={p.onResume} />
            <MenuItem label="take a screenshot" keys="F12" onClick={p.onScreenshot} />
            <MenuItem label={p.fullscreen ? "leave full screen" : "full screen"} keys="F11" onClick={p.onFullscreen} />
            <MenuItem label="restart game" onClick={p.onRestart} />
            <MenuItem label="quit to library" onClick={p.onQuit} />
          </nav>
        </section>

        <section className={s.settingsCol}>
          <h2 className={s.settingsTitle}>sound</h2>
          <div className={s.volumeRow}>
            <Slider label="Volume" value={p.muted ? 0 : p.volume} onChange={(v) => {
              if (p.muted && v > 0) p.onMute(false);
              p.onVolume(v);
            }} />
            <span className={s.volumeValue}>{p.muted ? "muted" : p.volume}</span>
          </div>
          <Toggle label="mute" checked={p.muted} onChange={p.onMute} />

          <h2 className={s.settingsTitle}>this game</h2>
          <Field label="scale">
            <Segmented<ScaleMode>
              label="Scale"
              value={game.scaleMode}
              onChange={(v) => p.onPatch({ scaleMode: v }, false)}
              options={[
                { value: "fit", label: "fit" },
                { value: "stretch", label: "stretch" },
                { value: "original", label: "original" },
              ]}
            />
          </Field>
          <Field label="behind the game">
            <Segmented
              label="Letterbox color"
              value={game.letterbox ?? "#000000"}
              onChange={(v) => p.onPatch({ letterbox: v === "#000000" ? "" : v }, false)}
              options={letterboxes.map((l) => ({
                value: l.value,
                label: (
                  <>
                    <span className={s.swatch} style={{ background: l.value }} />
                    {l.label}
                  </>
                ),
              }))}
            />
          </Field>
          <Field label="quality" restart>
            <Segmented<Quality>
              label="Quality"
              value={game.quality}
              onChange={(v) => p.onPatch({ quality: v }, true)}
              options={[
                { value: "low", label: "low" },
                { value: "medium", label: "medium" },
                { value: "high", label: "high" },
                { value: "best", label: "best" },
              ]}
            />
          </Field>
          <Field label="frame rate" restart>
            <Segmented
              label="Frame rate"
              value={fr}
              onChange={(v) => p.onPatch({ frameRateOverride: v === "auto" ? 0 : Number(v) }, true)}
              options={FRAME_RATES.map((f) => ({ value: f, label: f === "auto" ? `auto (${Math.round(game.frameRate)})` : f }))}
            />
          </Field>
          <Field label="run with" restart>
            <Segmented
              label="Runtime"
              value={game.runtime}
              onChange={(v) => p.onPatch({ runtime: v as Game["runtime"] }, true)}
              options={[
                { value: "ruffle", label: "ruffle" },
                { value: "projector", label: "flash projector" },
              ]}
            />
          </Field>
          <Toggle
            label="allow internet"
            hint="Some games phone home for high scores or level packs. Off keeps them offline. Applies on restart."
            checked={game.allowNetwork}
            onChange={(v) => p.onPatch({ allowNetwork: v }, true)}
          />

          {p.pendingRestart && (
            <div className={s.restartNote}>
              <span>Some changes apply when the game restarts. Your saves are kept.</span>
              <Button variant="accent" onClick={p.onRestart}>
                restart now
              </Button>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function MenuItem({
  label,
  keys,
  onClick,
  first,
}: {
  label: string;
  keys?: string;
  onClick: () => void;
  first?: React.RefObject<HTMLButtonElement | null>;
}) {
  return (
    <button className={s.menuItem} onClick={onClick} ref={first} data-nav>
      <span>{label}</span>
      {keys && <kbd>{keys}</kbd>}
    </button>
  );
}

function Field({ label, restart, children }: { label: string; restart?: boolean; children: React.ReactNode }) {
  return (
    <div className={s.field}>
      <span className={s.fieldLabel}>
        {label}
        {restart && <span className={s.fieldNote}>on restart</span>}
      </span>
      {children}
    </div>
  );
}
