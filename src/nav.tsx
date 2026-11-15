import { createContext, useContext } from "react";
import type { Route } from "./routes";

export interface Nav {
  navigate: (to: Route) => void;
  play: (gameId: number) => void;
  /** Opens a game's page. Pass the clicked element to grow the page out of it. */
  openGame: (gameId: number, from?: Element | null) => void;
  openCollection: (id: number) => void;
  /** Back to wherever you came from (Esc / Backspace do the same). */
  back: () => void;
}

export const NavContext = createContext<Nav>({
  navigate: () => {},
  play: () => {},
  openGame: () => {},
  openCollection: () => {},
  back: () => {},
});

export function useNav(): Nav {
  return useContext(NavContext);
}
