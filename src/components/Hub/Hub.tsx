import { useRef, type ReactNode, type WheelEvent } from "react";
import { Icon } from "../Icon/Icon";
import s from "./Hub.module.css";

// Panoramic hub: groups laid out side by side, scrolled horizontally.
// A vertical mouse wheel scrolls sideways, like the Windows 8 start screen.
export function Hub({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    const el = ref.current;
    if (!el || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    el.scrollLeft += e.deltaY;
  };

  return (
    <div className={s.hub} ref={ref} onWheel={onWheel}>
      <div className={s.track}>{children}</div>
    </div>
  );
}

export function HubGroup({
  title,
  index = 0,
  onTitleClick,
  children,
  plain = false,
}: {
  title: string;
  index?: number;
  onTitleClick?: () => void;
  children: ReactNode;
  /** plain groups lay children out freely instead of on the tile grid */
  plain?: boolean;
}) {
  return (
    <section className={`${s.group} ts`} style={{ ["--i" as string]: index }}>
      <h2 className={s.groupTitle}>
        {onTitleClick ? (
          <button onClick={onTitleClick} data-nav>
            {title}
            <Icon name="chevron" size={18} />
          </button>
        ) : (
          title
        )}
      </h2>
      <div className={plain ? s.plain : s.tiles}>{children}</div>
    </section>
  );
}
