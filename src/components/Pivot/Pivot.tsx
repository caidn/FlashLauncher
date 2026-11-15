import type { Route } from "../../routes";
import { Icon } from "../Icon/Icon";
import s from "./Pivot.module.css";

const ITEMS: { route: Route; label: string }[] = [
  { route: "home", label: "home" },
  { route: "library", label: "library" },
  { route: "collections", label: "collections" },
  { route: "browse", label: "browse" },
];

export function Pivot({ route, onNavigate }: { route: Route; onNavigate: (r: Route) => void }) {
  return (
    <nav className={s.pivot} aria-label="Main">
      <div className={s.items} role="tablist">
        {ITEMS.map((it) => (
          <button
            key={it.route}
            role="tab"
            aria-selected={route === it.route}
            className={s.item}
            data-active={route === it.route}
            data-nav
            onClick={() => onNavigate(it.route)}
          >
            {it.label}
          </button>
        ))}
      </div>
      <div className={s.actions}>
        <button
          className={s.action}
          data-active={route === "add"}
          data-nav
          onClick={() => onNavigate("add")}
          aria-label="Add games"
        >
          <Icon name="plus" size={22} />
          <span>add</span>
        </button>
        <button
          className={s.action}
          data-active={route === "settings"}
          data-nav
          onClick={() => onNavigate("settings")}
          aria-label="Settings"
        >
          <Icon name="gear" size={22} />
          <span>settings</span>
        </button>
      </div>
    </nav>
  );
}
