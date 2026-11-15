import { useState } from "react";
import { useLibrary } from "../../data/LibraryProvider";
import { Button, Checkbox, TextField } from "../Controls/Controls";
import { BandDialog } from "../Dialog/BandDialog";
import s from "./AddToCollection.module.css";

/**
 * Put games into collections (or take them out). A box is checked when
 * every chosen game is already in that collection.
 */
export function AddToCollection({ gameIds, onClose }: { gameIds: number[]; onClose: () => void }) {
  const { collections, addToCollection, removeFromCollection, createCollection } = useLibrary();
  const initial = Object.fromEntries(collections.map((c) => [c.id, gameIds.every((g) => c.gameIds.includes(g))]));
  const [checked, setChecked] = useState<Record<number, boolean>>(initial);
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    for (const c of collections) {
      if (checked[c.id] && !initial[c.id]) await addToCollection(c.id, gameIds);
      if (!checked[c.id] && initial[c.id]) await removeFromCollection(c.id, gameIds);
    }
    if (newName.trim()) await createCollection(newName.trim(), gameIds);
    setBusy(false);
    onClose();
  };

  return (
    <BandDialog
      title={gameIds.length === 1 ? "collections" : `collections for ${gameIds.length} games`}
      onCancel={onClose}
      actions={
        <>
          <Button variant="accent" onClick={save} disabled={busy}>
            save
          </Button>
          <Button onClick={onClose}>cancel</Button>
        </>
      }
    >
      {collections.length > 0 && (
        <ul className={s.list}>
          {collections.map((c) => (
            <li key={c.id}>
              <Checkbox
                label={c.name}
                checked={!!checked[c.id]}
                onChange={(v) => setChecked((prev) => ({ ...prev, [c.id]: v }))}
              />
              <span className={s.name} onClick={() => setChecked((prev) => ({ ...prev, [c.id]: !prev[c.id] }))}>
                {c.name}
              </span>
              <span className={s.count}>{c.gameIds.length}</span>
            </li>
          ))}
        </ul>
      )}
      <div className={s.newRow}>
        <span className={s.newLabel}>new collection</span>
        <TextField label="New collection name" value={newName} onChange={setNewName} onEnter={save} autoFocus={!collections.length} />
      </div>
    </BandDialog>
  );
}
