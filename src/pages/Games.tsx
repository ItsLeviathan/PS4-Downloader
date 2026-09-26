import { useEffect, useRef } from "react";
import { ApiKeyForm, GetKeyLink } from "../components/ApiKeyForm";
import { Button } from "../components/Button";
import { ErrorNotice } from "../components/ErrorNotice";
import { GameDetail } from "../components/GameDetail";
import { Icon } from "../components/Icon";
import { Metascore } from "../components/Metascore";
import { EmptyState, PageHeader } from "../components/Layout";
import { useApp } from "../stores/app";
import { useGames } from "../stores/games";
import type { GameSummary, Page, PlatformFilter } from "../types";

const SEARCH_DELAY_MS = 400;
const PLATFORMS: { value: PlatformFilter; label: string }[] = [
  { value: "ps4", label: "PS4" },
  { value: "ps5", label: "PS5" },
  { value: "all", label: "All platforms" },
];

export function Games({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const hasKey = useApp((s) => !!s.settings?.rawgApiKey);
  const selectedId = useGames((s) => s.selectedId);

  if (selectedId !== null) return <GameDetail id={selectedId} onNavigate={onNavigate} />;

  return (
    <div className="page">
      <PageHeader title="Games" subtitle="Search for a game to see its details before you download." />
      {hasKey ? <GameSearch /> : <KeySetup />}
    </div>
  );
}

function KeySetup() {
  return (
    <section className="card key-card">
      <div className="key-card-icon">
        <Icon name="key" size={22} />
      </div>
      <div className="stack">
        <h2>Connect the game database</h2>
        <p className="muted">
          Game search uses RAWG, a free video game database. Create a free account, copy your API key, and paste it
          here. You only need to do this once.
        </p>
        <ApiKeyForm />
        <GetKeyLink />
      </div>
    </section>
  );
}

function GameSearch() {
  const { query, platform, results, count, nextPage, searching, loadingMore, error } = useGames();
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

      {error && (
        <div className="stack">
          <ErrorNotice error={error} />
          {error.code === "apiKey" && <ApiKeyForm onSaved={search} />}
        </div>
      )}

      {!trimmed ? (
        <EmptyState icon="gamepad" title="Find a game">
          Type a name to see matching games with cover art, release date, ratings and more.
        </EmptyState>
      ) : results.length === 0 ? (
        !searching &&
        !error && (
          <EmptyState icon="search" title={`No games found for “${trimmed}”`}>
            Check the spelling, or try All platforms.
          </EmptyState>
        )
      ) : (
        <>
          <p className="muted small results-count" aria-live="polite">
            {count.toLocaleString()} {count === 1 ? "result" : "results"}
          </p>
          <ul className="game-grid">
            {results.map((game) => (
              <li key={game.id}>
                <GameCard game={game} onOpen={() => open(game.id)} />
              </li>
            ))}
          </ul>
          {nextPage && (
            <div className="load-more">
              <Button busy={loadingMore} onClick={loadMore}>
                Show more results
              </Button>
            </div>
          )}
        </>
      )}

      <p className="muted small attribution">Game data from RAWG.</p>
    </>
  );
}

function GameCard({ game, onOpen }: { game: GameSummary; onOpen: () => void }) {
  const year = game.released?.slice(0, 4) ?? (game.tba ? "TBA" : null);
  return (
    <button type="button" className="game-card" onClick={onOpen}>
      <div className="game-cover">
        {game.image ? (
          <img src={game.image} alt="" loading="lazy" />
        ) : (
          <Icon name="gamepad" size={32} className="game-cover-placeholder" />
        )}
        {game.metacritic !== null && <Metascore score={game.metacritic} />}
      </div>
      <div className="game-card-body">
        <p className="game-card-title">{game.name}</p>
        <p className="muted small game-card-meta">
          {[year, game.genres.slice(0, 2).join(", ")].filter(Boolean).join(" · ")}
        </p>
      </div>
    </button>
  );
}
