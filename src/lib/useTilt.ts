import { useCallback, useRef, type PointerEvent } from "react";
import { useReducedMotion } from "../theme/ThemeProvider";

// Windows 8 "pointer down" feedback: press near an edge and the element tilts
// away from the pointer; press near the middle and it just sinks a little.
export function useTilt<T extends HTMLElement>(max = 11) {
  const ref = useRef<T>(null);
  const reduced = useReducedMotion();

  const onPointerDown = useCallback(
    (e: PointerEvent<T>) => {
      const el = ref.current;
      if (!el || reduced || e.button !== 0) return;
      const r = el.getBoundingClientRect();
      const x = (e.clientX - r.left) / r.width - 0.5;
      const y = (e.clientY - r.top) / r.height - 0.5;
      el.style.transition = "transform 90ms var(--ease-out)";
      if (Math.abs(x) < 0.18 && Math.abs(y) < 0.18) {
        el.style.transform = "perspective(800px) scale(0.965)";
      } else {
        // Larger tiles tilt less, like the real thing.
        const k = max * Math.min(1, 220 / Math.max(r.width, r.height));
        el.style.transform = `perspective(800px) rotateX(${(-y * 2 * k).toFixed(2)}deg) rotateY(${(x * 2 * k).toFixed(2)}deg) scale(0.985)`;
      }
    },
    [max, reduced],
  );

  const release = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    el.style.transition = "transform var(--dur) var(--ease-out)";
    el.style.transform = "";
  }, []);

  return {
    ref,
    onPointerDown,
    onPointerUp: release,
    onPointerLeave: release,
    onPointerCancel: release,
  };
}
