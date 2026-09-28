  import { useEffect, useRef } from "react";
import { Button } from "../components/Button";
import { ErrorNotice } from "../components/ErrorNotice";
import { GameDetail } from "../components/GameDetail";
import { Icon } from "../components/Icon";
import { EmptyState, PageHeader } from "../components/Layout";
import { Metascore } from "../components/Metascore";
import { useGames } from "../stores/games";
import type { GameSummary, Page, PlatformFilter } from "../types";

const SEARCH_DELAY_MS = 350;
const SKELETON_CARDS = 10;
const PLATFORMS: { value: PlatformFilter; label: string }[] = [
  { value: "ps4", label: "PS4" },
  { value: "ps5", label: "PS5" },
  { value: "all", label: "All platforms" },
];

export function Games({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const selectedId = useGames((s) => s.selectedId);
  if (selectedId !== null) return <GameDetail id={selectedId} onNavigate={onNavigate} />;
  return (
    <div className="page">
      <PageHeader title="Games" subtitle="Search any game to see its full details before you download." />
      <GameSearch />
    </div>
  );
}

function GameSearch() {
  const { query, platform, results, nextOffset, searching, loadingMore, error } = useGames();
  const { setQuery, setPlatform, search, loadMore, open } = useGames.getState();
  const inputRef = useRef<HTMLInputElement>(null);
  const first = useRef(true);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Search as the user types; results stay when coming back from a game.
  useEffect(() => {
    if (first.current) {
      first.current = false;
      if (results.length || !query.trim()) return;
    }
    const timer = setTimeout(search, SEARCH_DELAY_MS);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, platform]);

  const trimmed = query.trim();
  const showSkeleton = searching && results.length === 0;

  return (
    <>
      <div className="card game-search">
        <label className="url-field">
          {searching ? <span className="spinner url-icon" aria-hidden="true" /> : <Icon name="search" size={18} className="url-icon" />}
          <input
            ref={inputRef}
            type="search"
            spellCheck={false}
            autoComplete="off"
            placeholder="Search games, e.g. Bloodborne"
            aria-label="Search games"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && search()}
          />
          {query && (
            <button type="button" className="icon-button" aria-label="Clear search" onClick={() => setQuery("")}>
              <Icon name="x" size={15} />
            </button>
          )}
        </label>
        <div className="segmented" role="radiogroup" aria-label="Platform">
          {PLATFORMS.map((p) => (
            <button key={p.value} type="button" role="radio" aria-checked={platform === p.value} onClick={() => setPlatform(p.value)}>
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {error && <ErrorNotice error={error} />}

      {!trimmed ? (
        <EmptyState icon="gamepad" title="Find a game">
          Type a name to see matching games with box art, release date, critic scores and more.
        </EmptyState>
      ) : showSkeleton ? (
        <ul className="game-grid" aria-busy="true" aria-label="Loading results">
          {Array.from({ length: SKELETON_CARDS }, (_, i) => (
            <li key={i} className="game-card skeleton" aria-hidden="true">
              <div className="game-cover" />
              <div className="game-card-body">
                <span className="skeleton-line" />
                <span className="skeleton-line short" />
              </div>
            </li>
          ))}
        </ul>
      ) : results.length === 0 ? (
        !searching &&
        !error && (
          <EmptyState icon="search" title={`No games found for “${trimmed}”`}>
            Check the spelling, try the full title, or switch to All platforms.
          </EmptyState>
        )
      ) : (
        <>
          <ul className={`game-grid${searching ? " is-refreshing" : ""}`}>
            {results.map((game) => (
              <li key={game.id}>
                <GameCard game={game} onOpen={() => open(game.id)} />
              </li>
            ))}
          </ul>
          {nextOffset !== null && (
            <div className="load-more">
              <Button busy={loadingMore} onClick={loadMore}>
                Show more results
              </Button>
            </div>
          )}
        </>
      )}

      <p className="muted small attribution">Game information from Wikipedia and Wikidata.</p>
    </>
  );
}

function GameCard({ game, onOpen }: { game: GameSummary; onOpen: () => void }) {
  const year = game.released?.slice(0, 4);
  return (
    <button type="button" className="game-card" onClick={onOpen}>
      <div className="game-cover">
        {game.image ? (
          <img src={game.image} alt="" loading="lazy" />
        ) : (
          <span className="game-cover-placeholder">
            <Icon name="gamepad" size={30} />
            <span>{game.name}</span>
          </span>
        )}
        {game.criticScore && <Metascore critic={game.criticScore} />}
        {game.consoles.length > 0 && (
          <span className="console-badges">
            {game.consoles.map((c) => (
              <span key={c} className="console-badge">
                {c}
              </span>
            ))}
          </span>
        )}
      </div>
      <div className="game-card-body">
        <p className="game-card-title">{game.name}</p>
        <p className="muted small game-card-meta">{[year, game.genres[0]].filter(Boolean).join(" · ")}</p>
      </div>
    </button>
  );
}
