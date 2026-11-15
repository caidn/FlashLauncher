import { useEffect, useState } from "react";
import { isTauri, windowControls } from "../../lib/tauri";
import { Icon } from "../Icon/Icon";
import s from "./TitleBar.module.css";

export function TitleBar({ subtitle }: { subtitle?: string }) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let off = () => {};
    const sync = () => windowControls.isMaximized().then(setMaximized);
    sync();
    windowControls.onResized(sync).then((fn) => (off = fn));
    return () => off();
  }, []);

  return (
    <header className={s.bar} data-tauri-drag-region>
      <span className={s.title} data-tauri-drag-region>
        flash launcher
        {subtitle && <span className={s.subtitle} data-tauri-drag-region>{subtitle}</span>}
      </span>
      {isTauri && (
        <div className={s.controls}>
          <button className={s.ctl} aria-label="Minimize" onClick={() => windowControls.minimize()}>
            <Icon name="minimize" size={14} />
          </button>
          <button
            className={s.ctl}
            aria-label={maximized ? "Restore" : "Maximize"}
            onClick={() => windowControls.toggleMaximize()}
          >
            <Icon name={maximized ? "restore" : "maximize"} size={12} />
          </button>
          <button className={`${s.ctl} ${s.close}`} aria-label="Close" onClick={() => windowControls.close()}>
            <Icon name="close" size={14} />
          </button>
        </div>
      )}
    </header>
  );
}
