import { Button } from "../../components/Controls/Controls";
import { Icon } from "../../components/Icon/Icon";
import { useImport } from "../../import/ImportProvider";
import { useNav } from "../../nav";
import { useTilt } from "../../lib/useTilt";
import p from "../page.module.css";
import s from "./AddGame.module.css";

export function AddGame() {
  const { browseFiles, scanFolder, busy } = useImport();
  const { navigate } = useNav();
  const tilt = useTilt<HTMLButtonElement>(6);

  return (
    <div className={p.page}>
      <h1 className={`${p.title} ts`} style={{ ["--i" as string]: 0 }}>
        add games
      </h1>
      <div className="ts" style={{ ["--i" as string]: 1 }}>
        <button {...tilt} className={s.drop} onClick={browseFiles} disabled={busy} data-nav>
          <Icon name="upload" size={48} />
          <span className={s.dropTitle}>drop .swf files anywhere</span>
          <span className={s.dropSub}>
            Drag them onto this window, folders too. Or click here to pick files.
          </span>
        </button>
      </div>
      <div className={`${s.buttons} ts`} style={{ ["--i" as string]: 2 }}>
        <Button variant="accent" onClick={browseFiles} disabled={busy}>
          <Icon name="folder" size={16} /> browse files
        </Button>
        <Button onClick={scanFolder} disabled={busy}>
          scan a folder
        </Button>
        <Button onClick={() => navigate("browse")}>
          <Icon name="download" size={16} /> download from Flashpoint
        </Button>
      </div>

      <div className={`${p.section} ts`} style={{ ["--i" as string]: 3 }}>
        <h2 className={p.sectionTitle}>what happens on import</h2>
        <ol className={s.steps}>
          <li>Each file is read and hashed, so a game you already have gets flagged instead of added twice.</li>
          <li>The header tells us the stage size, frame rate and ActionScript version, and any images inside get pulled out for placeholder art.</li>
          <li>You check the titles, then the files are copied into the launcher's own folder. Moving or deleting the originals afterwards won't break anything.</li>
        </ol>
      </div>
    </div>
  );
}
