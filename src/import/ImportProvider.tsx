import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Button, Checkbox, TextField } from "../components/Controls/Controls";
import { BandDialog } from "../components/Dialog/BandDialog";
import { ProgressBar, ProgressDots, useToast } from "../components/Feedback/Feedback";
import { useLibrary } from "../data/LibraryProvider";
import type { ImportCandidate, ImportProgress, ImportSummary } from "../data/types";
import { api } from "../lib/api";
import { formatBytes, formatFps } from "../lib/format";
import { useScrape } from "../art/ScrapeProvider";
import { DropOverlay } from "./DropOverlay";
import s from "./Import.module.css";

type Stage =
  | { kind: "idle" }
  | { kind: "inspecting" }
  | { kind: "review"; rows: Row[] }
  | { kind: "importing"; total: number }
  | { kind: "failed"; summary: ImportSummary };

interface Row {
  c: ImportCandidate;
  title: string;
  include: boolean;
}

interface ImportApi {
  /** Start an import from a list of files and/or folders. */
  start: (paths: string[]) => void;
  browseFiles: () => void;
  scanFolder: () => void;
  busy: boolean;
}

const Ctx = createContext<ImportApi | null>(null);

function importable(c: ImportCandidate) {
  return !c.error && !c.duplicateOf && !c.duplicateInBatch;
}

export function ImportProvider({ children, onImported }: { children: ReactNode; onImported?: () => void }) {
  const [stage, setStage] = useState<Stage>({ kind: "idle" });
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [dragging, setDragging] = useState(false);
  const { addGames } = useLibrary();
  const { scrapeGames } = useScrape();
  const toast = useToast();
  const busyRef = useRef(false);
  busyRef.current = stage.kind !== "idle";

  useEffect(() => {
    let off = () => {};
    api.onProgress(setProgress).then((fn) => (off = fn));
    return () => off();
  }, []);

  const start = useCallback(
    async (paths: string[]) => {
      if (!paths.length || busyRef.current) return;
      setProgress(null);
      setStage({ kind: "inspecting" });
      try {
        const found = await api.inspect(paths);
        if (!found.length) {
          setStage({ kind: "idle" });
          toast({ title: "no swf files there", body: "Nothing in what you picked ends in .swf." });
          return;
        }
        setStage({
          kind: "review",
          rows: found.map((c) => ({ c, title: c.title, include: importable(c) })),
        });
      } catch (e) {
        setStage({ kind: "idle" });
        toast({ title: "couldn't read those files", body: String(e) });
      }
    },
    [toast],
  );

  // End-to-end tests can't drag files in or click native dialogs, so a
  // build made with VITE_E2E=1 exposes the import entry point.
  useEffect(() => {
    if (import.meta.env.VITE_E2E) (window as unknown as Record<string, unknown>).__e2eImport = start;
  }, [start]);

  // Files dropped anywhere on the window.
  useEffect(() => {
    let off = () => {};
    api
      .onFileDrop({
        enter: () => !busyRef.current && setDragging(true),
        leave: () => setDragging(false),
        drop: (paths) => {
          setDragging(false);
          start(paths);
        },
      })
      .then((fn) => (off = fn));
    return () => off();
  }, [start]);

  const browseFiles = useCallback(async () => {
    start(await api.pickFiles());
  }, [start]);

  const scanFolder = useCallback(async () => {
    const dir = await api.pickFolder();
    if (dir) start([dir]);
  }, [start]);

  const commit = async (rows: Row[]) => {
    const items = rows.filter((r) => r.include).map((r) => ({ path: r.c.path, title: r.title.trim() || r.c.title }));
    if (!items.length) return setStage({ kind: "idle" });
    setProgress(null);
    setStage({ kind: "importing", total: items.length });
    try {
      const summary = await api.importGames(items);
      addGames(summary.imported);
      // Look up art and info for the new games in the background.
      scrapeGames(summary.imported.map((g) => g.id));
      const n = summary.imported.length;
      if (n) {
        toast({
          title: n === 1 ? `added ${summary.imported[0].title}` : `added ${n} games`,
          body: summary.failed.length
            ? `${summary.failed.length} couldn't be added.`
            : "Looking up art and info for them now.",
        });
        onImported?.();
      }
      setStage(summary.failed.length ? { kind: "failed", summary } : { kind: "idle" });
    } catch (e) {
      setStage({ kind: "idle" });
      toast({ title: "import failed", body: String(e) });
    }
  };

  return (
    <Ctx.Provider value={{ start, browseFiles, scanFolder, busy: stage.kind !== "idle" }}>
      {children}
      {dragging && <DropOverlay />}
      {stage.kind === "inspecting" && (
        <BandDialog title="reading files">
          <ProgressDots />
          <p className={s.progressText}>
            {progress && progress.total > 0
              ? `${Math.min(progress.done + 1, progress.total)} of ${progress.total}${progress.current ? ` · ${progress.current}` : ""}`
              : "Looking for SWFs"}
          </p>
        </BandDialog>
      )}
      {stage.kind === "review" && (
        <Review
          rows={stage.rows}
          onChange={(rows) => setStage({ kind: "review", rows })}
          onCancel={() => setStage({ kind: "idle" })}
          onConfirm={commit}
        />
      )}
      {stage.kind === "importing" && (
        <BandDialog title={`adding ${stage.total} ${stage.total === 1 ? "game" : "games"}`}>
          <ProgressBar value={progress?.phase === "import" ? progress.done : 0} max={stage.total} />
          <p className={s.progressText}>{progress?.current || "Copying into your library"}</p>
        </BandDialog>
      )}
      {stage.kind === "failed" && (
        <BandDialog
          title={`${stage.summary.failed.length} ${stage.summary.failed.length === 1 ? "file" : "files"} couldn't be added`}
          onCancel={() => setStage({ kind: "idle" })}
          actions={
            <Button variant="accent" onClick={() => setStage({ kind: "idle" })} data-autofocus>
              ok
            </Button>
          }
        >
          <ul className={s.failList}>
            {stage.summary.failed.map((f) => (
              <li key={f.path}>
                <span className={s.failName}>{f.path.split(/[\\/]/).pop()}</span>
                <span>{f.error}</span>
              </li>
            ))}
          </ul>
        </BandDialog>
      )}
    </Ctx.Provider>
  );
}

function Review({
  rows,
  onChange,
  onCancel,
  onConfirm,
}: {
  rows: Row[];
  onChange: (rows: Row[]) => void;
  onCancel: () => void;
  onConfirm: (rows: Row[]) => void;
}) {
  const count = rows.filter((r) => r.include).length;
  const set = (i: number, patch: Partial<Row>) => onChange(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const blocked = rows.filter((r) => !importable(r.c)).length;
  const emptyTitle = rows.some((r) => r.include && !r.title.trim());

  return (
    <BandDialog
      wide
      tall={rows.length > 3}
      title={count === 1 ? "add 1 game" : `add ${count} games`}
      onCancel={onCancel}
      actions={
        <>
          <Button variant="accent" disabled={!count || emptyTitle} onClick={() => onConfirm(rows)}>
            {count === 1 ? "add game" : `add ${count} games`}
          </Button>
          <Button onClick={onCancel}>cancel</Button>
        </>
      }
    >
      <p className={s.lead}>
        Check the titles. They're used to find art and info later, so fix any that came out wrong.
        {blocked > 0 && ` ${blocked} can't be added and ${blocked === 1 ? "is" : "are"} unchecked.`}
      </p>
      <div className={s.rows}>
        {rows.map((r, i) => (
          <ReviewRow key={r.c.path} row={r} index={i} onChange={(p) => set(i, p)} />
        ))}
      </div>
    </BandDialog>
  );
}

function ReviewRow({ row, index, onChange }: { row: Row; index: number; onChange: (p: Partial<Row>) => void }) {
  const { c } = row;
  const ok = importable(c);
  const note = c.error
    ? c.error
    : c.duplicateOf
      ? `already in your library as "${c.duplicateOf.title}"`
      : c.duplicateInBatch
        ? "same file as one above"
        : null;
  const i = c.info;

  return (
    <div className={s.row} style={{ ["--i" as string]: index }} data-ok={ok}>
      <Checkbox checked={row.include} disabled={!ok} label={`Include ${c.fileName}`} onChange={(v) => onChange({ include: v })} />
      <div className={s.thumb} data-empty={!c.preview}>
        {c.preview ? <img src={api.fileUrl(c.preview)} alt="" /> : <span>{i ? `as${i.asVersion}` : "?"}</span>}
      </div>
      <div className={s.fields}>
        {ok ? (
          <TextField
            label={`Title for ${c.fileName}`}
            value={row.title}
            invalid={row.include && !row.title.trim()}
            onChange={(v) => onChange({ title: v })}
          />
        ) : (
          <span className={s.blockedTitle}>{row.title}</span>
        )}
        <span className={s.meta}>
          <span className={s.file}>{c.fileName}</span>
          {i && (
            <>
              <span>AS{i.asVersion}</span>
              <span>
                {Math.round(i.width)}×{Math.round(i.height)}
              </span>
              <span>{formatFps(i.frameRate)}</span>
              <span>swf {i.version}</span>
            </>
          )}
          {c.fileSize > 0 && <span>{formatBytes(c.fileSize)}</span>}
          {c.imageCount > 0 && (
            <span>
              {c.imageCount} {c.imageCount === 1 ? "image" : "images"} inside
            </span>
          )}
        </span>
        {note && <span className={s.note}>{note}</span>}
      </div>
    </div>
  );
}

export function useImport(): ImportApi {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useImport must be used inside ImportProvider");
  return ctx;
}
