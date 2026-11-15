import { createPortal } from "react-dom";
import { Icon } from "../components/Icon/Icon";
import s from "./Import.module.css";

export function DropOverlay() {
  return createPortal(
    <div className={s.drop} aria-hidden="true">
      <div className={s.dropInner}>
        <Icon name="upload" size={56} />
        <span className={s.dropTitle}>drop to add</span>
        <span className={s.dropSub}>SWF files, or folders full of them</span>
      </div>
    </div>,
    document.getElementById("layer") ?? document.body,
  );
}
