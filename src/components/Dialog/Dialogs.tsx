import { useState, type ReactNode } from "react";
import { Button, TextField } from "../Controls/Controls";
import { BandDialog } from "./BandDialog";

/** Asks for one line of text (rename, new collection, ...). */
export function PromptDialog({
  title,
  note,
  label,
  initial = "",
  confirmLabel = "save",
  onSave,
  onClose,
}: {
  title: string;
  note?: ReactNode;
  label: string;
  initial?: string;
  confirmLabel?: string;
  onSave: (value: string) => void;
  onClose: () => void;
}) {
  const [value, setValue] = useState(initial);
  const valid = value.trim().length > 0;
  const save = () => valid && onSave(value.trim());
  return (
    <BandDialog
      title={title}
      onCancel={onClose}
      actions={
        <>
          <Button variant="accent" disabled={!valid} onClick={save}>
            {confirmLabel}
          </Button>
          <Button onClick={onClose}>cancel</Button>
        </>
      }
    >
      {note && <p style={{ fontSize: "var(--fs-small)", marginBottom: 10 }}>{note}</p>}
      <TextField label={label} value={value} onChange={setValue} autoFocus invalid={!valid && value !== initial} onEnter={save} />
    </BandDialog>
  );
}

/** Are-you-sure for things that can't be undone. */
export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  onConfirm,
  onClose,
}: {
  title: string;
  children?: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <BandDialog
      title={title}
      onCancel={onClose}
      actions={
        <>
          <Button variant="accent" data-autofocus onClick={onConfirm}>
            {confirmLabel}
          </Button>
          <Button onClick={onClose}>cancel</Button>
        </>
      }
    >
      {children}
    </BandDialog>
  );
}
