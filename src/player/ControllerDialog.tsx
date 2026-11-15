import { useEffect, useState } from "react";
import { Button, Segmented } from "../components/Controls/Controls";
import { BandDialog } from "../components/Dialog/BandDialog";
import { useLibrary } from "../data/LibraryProvider";
import type { Game } from "../data/types";
import { codeFromEvent, keyName, PAD_BUTTONS, padMapFor, presetOf, PRESETS, type PadButton, type PadMap } from "./gamepadMap";
import s from "./ControllerDialog.module.css";

/** Per-game controller layout: which key each button presses in the game. */
export function ControllerDialog({ game, onClose }: { game: Game; onClose: () => void }) {
  const { updateGame } = useLibrary();
  const [map, setMap] = useState<PadMap>(() => padMapFor(game));
  const [listening, setListening] = useState<PadButton | null>(null);
  const preset = presetOf(map);

  const save = (next: PadMap, isDefault = false) => {
    setMap(next);
    updateGame(game.id, { gamepadMap: isDefault ? "" : JSON.stringify(next) });
  };

  // Waiting for a key: the next key press is the binding, Esc cancels.
  useEffect(() => {
    if (!listening) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key !== "Escape") {
        const code = codeFromEvent(e);
        if (code != null) save({ ...map, [listening]: code });
      }
      setListening(null);
    };
    // Capture phase, ahead of the dialog's own Esc handler.
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listening, map]);

  return (
    <BandDialog
      title="controller"
      onCancel={listening ? undefined : onClose}
      wide
      tall
      actions={
        <>
          <Button onClick={() => save(PRESETS[0].map, true)}>reset</Button>
          <Button variant="accent" onClick={onClose} data-autofocus>
            done
          </Button>
        </>
      }
    >
      <p className={s.blurb}>
        Pick which key each button presses in {game.title}. Hold <b>view + menu</b> together (or press the guide
        button) to open the launcher's menu while playing. Mouse-only games can't use a controller.
      </p>
      <div className={s.presets}>
        <Segmented
          label="Layout"
          value={preset}
          onChange={(id) => {
            const p = PRESETS.find((x) => x.id === id);
            if (p) save(p.map, p.id === PRESETS[0].id);
          }}
          options={[...PRESETS.map((p) => ({ value: p.id, label: p.label })), { value: "custom", label: "custom" }]}
        />
      </div>
      <div className={s.grid}>
        {PAD_BUTTONS.map((b) => (
          <div key={b.id} className={s.row}>
            <span className={s.button}>{b.label}</span>
            <button
              className={s.key}
              data-listening={listening === b.id}
              data-empty={map[b.id] == null}
              onClick={() => setListening(b.id)}
              data-nav
            >
              {listening === b.id ? "press a key…" : keyName(map[b.id])}
            </button>
            <button
              className={s.clear}
              aria-label={`Clear ${b.label}`}
              disabled={map[b.id] == null}
              onClick={() => {
                const next = { ...map };
                delete next[b.id];
                save(next);
              }}
              data-nav
            >
              clear
            </button>
          </div>
        ))}
      </div>
    </BandDialog>
  );
}
