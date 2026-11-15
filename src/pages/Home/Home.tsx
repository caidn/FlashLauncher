import { FallbackArt } from "../../components/FallbackArt/FallbackArt";
import { Hub, HubGroup } from "../../components/Hub/Hub";
import { Icon } from "../../components/Icon/Icon";
import { Tile } from "../../components/Tile/Tile";
import { useLibrary } from "../../data/LibraryProvider";
import type { Game } from "../../data/types";
import { useImport } from "../../import/ImportProvider";
import { useNav } from "../../nav";
import { formatPlayTime, timeAgo } from "../../lib/format";
import { readableOn } from "../../theme/accents";
import type { Route } from "../../routes";
import s from "./Home.module.css";

interface Props {
  onNavigate: (r: Route) => void;
  isSample: boolean;
}

export function Home({ onNavigate, isSample }: Props) {
  const { games, collections, loaded } = useLibrary();
  const { browseFiles } = useImport();
  const { openGame, openCollection } = useNav();
  if (loaded && games.length === 0) return <EmptyHub onAdd={browseFiles} onNavigate={onNavigate} />;

  const recent = [...games]
    .filter((g) => g.lastPlayed)
    .sort((a, b) => b.lastPlayed!.localeCompare(a.lastPlayed!));
  const favorites = games.filter((g) => g.favorite);
  const byId = new Map(games.map((g) => [g.id, g]));

  // Until games have been played, "recent" means recently added.
  const recentList = recent.length
    ? recent
    : [...games].sort((a, b) => b.addedAt.localeCompare(a.addedAt));
  const [hero, second, ...rest] = recentList;

  return (
    <Hub>
      <HubGroup title={recent.length ? "recent" : "just added"} index={0} onTitleClick={() => onNavigate("library")}>
        {hero && (
          <Tile
            size="large"
            color={hero.color}
            ariaLabel={hero.title}
            onClick={(el) => openGame(hero.id, el)}
            interval={6}
            faces={[
              <FallbackArt game={hero} variant="square" />,
              <StatsFace game={hero} big />,
            ]}
          />
        )}
        {second && (
          <Tile
            size="wide"
            color={second.color}
            ariaLabel={second.title}
            onClick={(el) => openGame(second.id, el)}
            interval={8}
            faces={[<FallbackArt game={second} variant="tile" />, <StatsFace game={second} />]}
          />
        )}
        {rest.slice(0, 3).map((g) => (
          <Tile
            key={g.id}
            size="medium"
            color={g.color}
            ariaLabel={g.title}
            onClick={(el) => openGame(g.id, el)}
            interval={9}
            faces={[<FallbackArt game={g} variant="square" />, <StatsFace game={g} />]}
          />
        ))}
        {rest.slice(3, 7).map((g) => (
          <Tile
            key={g.id}
            size="small"
            color={g.color}
            ariaLabel={g.title}
            onClick={(el) => openGame(g.id, el)}
            faces={[<SmallFace game={g} />]}
          />
        ))}
      </HubGroup>

      {favorites.length > 0 && (
        <HubGroup title="favorites" index={1}>
          {favorites.map((g, i) => (
            <Tile
              key={g.id}
              size={i === 0 ? "wide" : "medium"}
              color={g.color}
              ariaLabel={g.title}
            onClick={(el) => openGame(g.id, el)}
              interval={10}
              faces={[<FallbackArt game={g} variant={i === 0 ? "tile" : "square"} />, <StatsFace game={g} />]}
            />
          ))}
        </HubGroup>
      )}

      {collections.length > 0 && (
        <HubGroup title="collections" index={2} onTitleClick={() => onNavigate("collections")}>
          {collections.map((c) => {
            const members = c.gameIds.map((id) => byId.get(id)).filter((g): g is Game => !!g);
            return (
              <Tile
                key={c.id}
                size="wide"
                ariaLabel={c.name}
              onClick={() => openCollection(c.id)}
                interval={11}
                faces={[
                  <MosaicFace games={members} name={c.name} />,
                  <CollectionNameFace name={c.name} count={members.length} />,
                ]}
              />
            );
          })}
        </HubGroup>
      )}

      <HubGroup title="start" index={3}>
        <Tile
          size="wide"
          label="add games"
          onClick={browseFiles}
          faces={[
            <span className={s.iconFace}>
              <Icon name="upload" size={44} />
            </span>,
          ]}
        />
        <Tile
          size="medium"
          label="library"
          badge={games.length}
          color="var(--bg-3)"
          style={{ color: "var(--fg)" }}
          onClick={() => onNavigate("library")}
          faces={[
            <span className={s.iconFace}>
              <Icon name="grid" size={36} />
            </span>,
          ]}
        />
        <Tile
          size="medium"
          label="settings"
          color="var(--bg-3)"
          style={{ color: "var(--fg)" }}
          onClick={() => onNavigate("settings")}
          faces={[
            <span className={s.iconFace}>
              <Icon name="gear" size={36} />
            </span>,
          ]}
        />
        {isSample && (
          <p className={s.note} style={{ gridColumn: "span 4", gridRow: "span 2" }}>
            Browser preview with sample games. The desktop app shows your real library.
          </p>
        )}
      </HubGroup>
    </Hub>
  );
}

function StatsFace({ game, big = false }: { game: Game; big?: boolean }) {
  return (
    <span className={s.stats} data-big={big} style={{ background: game.color, color: readableOn(game.color) }}>
      <span className={s.statsTime}>{game.playSeconds ? formatPlayTime(game.playSeconds) : "new"}</span>
      <span className={s.statsSub}>
        {game.lastPlayed ? `played · last ${timeAgo(game.lastPlayed)}` : `added ${timeAgo(game.addedAt)}`}
      </span>
      {big && (
        <span className={s.statsDev}>
          {[game.developer, game.year].filter(Boolean).join(", ") ||
            `${Math.round(game.width)}×${Math.round(game.height)} · as${game.asVersion}`}
        </span>
      )}
      <span className={s.statsTitle}>{game.title}</span>
    </span>
  );
}

function SmallFace({ game }: { game: Game }) {
  return (
    <span className={s.small} style={{ color: readableOn(game.color) }}>
      {game.title.charAt(0).toUpperCase()}
    </span>
  );
}

function MosaicFace({ games, name }: { games: Game[]; name: string }) {
  // Up to four covers side by side, like a shelf.
  const cells = Array.from({ length: 4 }, (_, i) => games[i]);
  return (
    <span className={s.mosaic}>
      {cells.map((g, i) =>
        g ? (
          <span key={i} className={s.mosaicCell}>
            <FallbackArt game={g} variant="cover" compact />
          </span>
        ) : (
          <span key={i} style={{ background: "var(--bg-3)" }} />
        ),
      )}
      <span className={s.mosaicName}>{name}</span>
    </span>
  );
}

function CollectionNameFace({ name, count }: { name: string; count: number }) {
  return (
    <span className={s.collectionName}>
      {name}
      <span className={s.statsSub}>
        {count} {count === 1 ? "game" : "games"}
      </span>
    </span>
  );
}

function EmptyHub({ onAdd, onNavigate }: { onAdd: () => void; onNavigate: (r: Route) => void }) {
  return (
    <Hub>
      <HubGroup title="start" index={0}>
        <Tile
          size="large"
          label="add games"
          onClick={onAdd}
          faces={[
            <span className={s.iconFace}>
              <Icon name="upload" size={72} />
            </span>,
          ]}
        />
        <Tile
          size="medium"
          label="settings"
          color="var(--bg-3)"
          style={{ color: "var(--fg)" }}
          onClick={() => onNavigate("settings")}
          faces={[
            <span className={s.iconFace}>
              <Icon name="gear" size={36} />
            </span>,
          ]}
        />
        <p className={s.welcome} style={{ gridColumn: "span 6", gridRow: "span 2" }}>
          <span className={s.welcomeTitle}>welcome</span>
          Add your SWFs to get going. Drag files or whole folders anywhere onto this window, or pick them with add games.
        </p>
      </HubGroup>
    </Hub>
  );
}
