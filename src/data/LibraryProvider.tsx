import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { useToast } from "../components/Feedback/Feedback";
import { api } from "../lib/api";
import type { Collection, Game, GamePatch } from "./types";

interface LibraryState {
  games: Game[];
  collections: Collection[];
  loaded: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  addGames: (games: Game[]) => void;
  updateGame: (id: number, patch: GamePatch) => Promise<void>;
  deleteGames: (ids: number[]) => Promise<void>;
  /** Re-reads one game from the backend (after play, art changes, ...). */
  reloadGame: (id: number) => Promise<void>;
  createCollection: (name: string, games?: number[]) => Promise<Collection | null>;
  renameCollection: (id: number, name: string) => Promise<void>;
  deleteCollection: (id: number) => Promise<void>;
  addToCollection: (id: number, games: number[]) => Promise<void>;
  removeFromCollection: (id: number, games: number[]) => Promise<void>;
}

const Ctx = createContext<LibraryState | null>(null);

export function LibraryProvider({ children }: { children: ReactNode }) {
  const [games, setGames] = useState<Game[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const toast = useToast();

  // Surface backend errors instead of failing silently.
  useEffect(() => {
    if (error) toast({ title: "something went wrong", body: error });
  }, [error, toast]);

  const refresh = useCallback(async () => {
    try {
      const [g, c] = await Promise.all([api.listGames(), api.listCollections()]);
      setGames(g);
      setCollections(c);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const addGames = useCallback((added: Game[]) => {
    setGames((prev) => [...prev, ...added]);
  }, []);

  const updateGame = useCallback(async (id: number, patch: GamePatch) => {
    // Optimistic: show the change now, reconcile with what the DB returns.
    setGames((prev) => prev.map((g) => (g.id === id ? { ...g, ...patch } : g)));
    try {
      const saved = await api.updateGame(id, patch);
      setGames((prev) => prev.map((g) => (g.id === id ? saved : g)));
    } catch (e) {
      setError(String(e));
      refresh();
    }
  }, [refresh]);

  const deleteGames = useCallback(async (ids: number[]) => {
    setGames((prev) => prev.filter((g) => !ids.includes(g.id)));
    try {
      for (const id of ids) await api.deleteGame(id);
    } catch (e) {
      setError(String(e));
    }
    refresh();
  }, [refresh]);

  const reloadGame = useCallback(async (id: number) => {
    try {
      const g = await api.getGame(id);
      setGames((prev) => prev.map((x) => (x.id === id ? g : x)));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Collection changes come back as the updated collection; swap it in.
  const putCollection = (c: Collection) =>
    setCollections((prev) => {
      const i = prev.findIndex((x) => x.id === c.id);
      const next = i === -1 ? [...prev, c] : prev.map((x) => (x.id === c.id ? c : x));
      return next.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
    });

  const guard = useCallback(async <T,>(fn: () => Promise<T>): Promise<T | null> => {
    try {
      return await fn();
    } catch (e) {
      setError(String(e));
      return null;
    }
  }, []);

  const createCollection = useCallback(
    async (name: string, gameIds: number[] = []) => {
      const c = await guard(() => api.createCollection(name, gameIds));
      if (c) putCollection(c);
      return c;
    },
    [guard],
  );
  const renameCollection = useCallback(
    async (id: number, name: string) => {
      const c = await guard(() => api.renameCollection(id, name));
      if (c) putCollection(c);
    },
    [guard],
  );
  const deleteCollection = useCallback(
    async (id: number) => {
      setCollections((prev) => prev.filter((c) => c.id !== id));
      await guard(() => api.deleteCollection(id));
    },
    [guard],
  );
  const addToCollection = useCallback(
    async (id: number, gameIds: number[]) => {
      const c = await guard(() => api.addToCollection(id, gameIds));
      if (c) putCollection(c);
    },
    [guard],
  );
  const removeFromCollection = useCallback(
    async (id: number, gameIds: number[]) => {
      const c = await guard(() => api.removeFromCollection(id, gameIds));
      if (c) putCollection(c);
    },
    [guard],
  );

  return (
    <Ctx.Provider
      value={{
        games,
        collections,
        loaded,
        error,
        refresh,
        addGames,
        updateGame,
        deleteGames,
        reloadGame,
        createCollection,
        renameCollection,
        deleteCollection,
        addToCollection,
        removeFromCollection,
      }}
    >
      {children}
    </Ctx.Provider>
  );
}

export function useLibrary(): LibraryState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useLibrary must be used inside LibraryProvider");
  return ctx;
}
