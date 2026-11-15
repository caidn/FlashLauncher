// Controller support for the launcher's own screens. Polls the standard
// gamepad mapping and turns it into the same things the keyboard does:
// d-pad / left stick move focus (spatial nav), A clicks, B is Esc (back,
// close dialogs), LB / RB switch pivot pages, Y opens the right-click
// action (select in the library).
//
// While a game runs, spatial nav is off and the game gets the controller
// through Ruffle's own mapping. Only View + Menu together (or the guide
// button) is caught here, to open the in-game menu.

import { isSpatialNavEnabled, moveFocus } from "./spatialNav";

const A = 0;
const B = 1;
const Y = 3;
const LB = 4;
const RB = 5;
const VIEW = 8;
const MENU = 9;
const UP = 12;
const DOWN = 13;
const LEFT = 14;
const RIGHT = 15;
const GUIDE = 16;

const FIRST_REPEAT_MS = 380;
const REPEAT_MS = 110;
const STICK = 0.55;

export interface GamepadHandlers {
  /** LB (-1) / RB (+1). */
  onShoulder?: (dir: -1 | 1) => void;
  /** View + Menu or guide while a game is running. */
  onGameMenu?: () => void;
}

type Dir = "up" | "down" | "left" | "right";

function key(name: string) {
  const target = (document.activeElement as HTMLElement | null) ?? document.body;
  target.dispatchEvent(new KeyboardEvent("keydown", { key: name, code: name, bubbles: true, cancelable: true }));
}

function setInput(kind: "gamepad" | "pointer") {
  if (document.documentElement.dataset.input !== kind) document.documentElement.dataset.input = kind;
}

export function installGamepad(h: GamepadHandlers): () => void {
  let raf = 0;
  let prev: boolean[] = [];
  let held: { dir: Dir; since: number; last: number } | null = null;
  let comboDown = false;

  const pressed = (pad: Gamepad, i: number) => !!pad.buttons[i]?.pressed;

  const dirOf = (pad: Gamepad): Dir | null => {
    const x = pad.axes[0] ?? 0;
    const y = pad.axes[1] ?? 0;
    if (pressed(pad, UP) || y < -STICK) return "up";
    if (pressed(pad, DOWN) || y > STICK) return "down";
    if (pressed(pad, LEFT) || x < -STICK) return "left";
    if (pressed(pad, RIGHT) || x > STICK) return "right";
    return null;
  };

  const tick = (now: number) => {
    raf = requestAnimationFrame(tick);
    const pads = navigator.getGamepads?.() ?? [];
    const pad = Array.from(pads).find((p): p is Gamepad => !!p && p.connected);
    if (!pad) return;
    const cur = pad.buttons.map((b) => b.pressed);
    const edge = (i: number) => cur[i] && !prev[i];
    const any = cur.some(Boolean) || Math.abs(pad.axes[0] ?? 0) > STICK || Math.abs(pad.axes[1] ?? 0) > STICK;

    // A game owns the controller; only the menu combo is ours.
    if (!isSpatialNavEnabled()) {
      const combo = (cur[VIEW] && cur[MENU]) || cur[GUIDE];
      if (combo && !comboDown) h.onGameMenu?.();
      comboDown = !!combo;
      prev = cur;
      held = null;
      return;
    }
    comboDown = false;
    if (any) setInput("gamepad");

    const dir = dirOf(pad);
    if (dir) {
      if (!held || held.dir !== dir) {
        held = { dir, since: now, last: now };
        step(dir);
      } else if (now - held.since > FIRST_REPEAT_MS && now - held.last > REPEAT_MS) {
        held.last = now;
        step(dir);
      }
    } else {
      held = null;
    }

    if (edge(A)) activate();
    if (edge(B)) back();
    if (edge(Y)) contextAction();
    if (edge(LB)) h.onShoulder?.(-1);
    if (edge(RB)) h.onShoulder?.(1);
    prev = cur;
  };

  const step = (dir: Dir) => {
    const el = document.activeElement as HTMLElement | null;
    // Inside a text box, left/right move the caret; up/down leave it.
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA") && (dir === "left" || dir === "right")) return;
    moveFocus(dir, el && el !== document.body ? el : null);
  };

  const activate = () => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) {
      moveFocus("down", null);
      return;
    }
    if (el.tagName === "INPUT") key("Enter");
    else el.click();
  };

  const back = () => {
    const el = document.activeElement as HTMLElement | null;
    // Esc is ignored inside text boxes, so step out of them first.
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) el.blur();
    key("Escape");
  };

  const contextAction = () => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return;
    const r = el.getBoundingClientRect();
    el.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 }),
    );
  };

  const start = () => {
    if (!raf) raf = requestAnimationFrame(tick);
  };
  const stop = () => {
    const left = Array.from(navigator.getGamepads?.() ?? []).some((p) => p?.connected);
    if (!left && raf) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
  };
  const onPointer = () => setInput("pointer");

  window.addEventListener("gamepadconnected", start);
  window.addEventListener("gamepaddisconnected", stop);
  window.addEventListener("pointermove", onPointer, { passive: true });
  // A controller that was already plugged in shows up on its first press.
  if (Array.from(navigator.getGamepads?.() ?? []).some((p) => p?.connected)) start();

  return () => {
    cancelAnimationFrame(raf);
    raf = 0;
    window.removeEventListener("gamepadconnected", start);
    window.removeEventListener("gamepaddisconnected", stop);
    window.removeEventListener("pointermove", onPointer);
  };
}
