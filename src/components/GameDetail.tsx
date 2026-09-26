import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { formatReleaseDate } from "../lib/format";
import { runAction } from "../stores/app";
import { useGames } from "../stores/games";
import type { GameDetails, Page } from "../types";
import { AddDownloadForm } from "./AddDownloadForm";
import { Button } from "./Button";
import { ErrorNotice } from "./ErrorNotice";
import { Icon } from "./Icon";
import { Metascore } from "./Metascore";

const DESCRIPTION_PREVIEW = 700;

export function GameDetail({ id, onNavigate }: { id: number; onNavigate: (page: Page) => void }) {
  const game = useGames((s) => s.details[id]);
  const error = useGames((s) => s.detailsError);
  const { close, open } = useGames.getState();

  useEffect(() => {
    document.querySelector(".content")?.scrollTo({ top: 0 });
  }, [id]);

  const back = (
    <div>
      <Button variant="ghost" icon="back" onClick={close}>
        Back to results
      </Button>
    </div>
  );

  if (error) {
    return (
      <div className="page">
        {back}
        <ErrorNotice error={error} />
        <div>
          <Button icon="retry" onClick={() => open(id)}>
            Try again
          </Button>
        </div>
      </div>
    );
  }

  if (!game) {
    return (
      <div className="page">
        {back}
        <div className="game-loading" aria-busy="true">
          <span className="spinner" aria-hidden="true" />
          <span className="muted">Loading game details…</span>
        </div>
      </div>
    );
  }

  return (
    <div className="page game-page">
      {back}
      <Hero game={game} />
      <div className="game-layout">
        <div className="stack game-main">
          <About game={game} />
          <Media game={game} />
        </div>
        <aside className="stack game-side">
          <Facts game={game} />
        </aside>
      </div>
      <section className="card stack game-download">
        <div className="card-head">
          <h2 className="card-title">
            <Icon name="download" size={18} /> Download
          </h2>
          <Button size="sm" variant="ghost" onClick={() => onNavigate("downloads")}>
            View downloads
          </Button>
        </div>
        <p className="muted small">
          Paste a direct link to a file you're allowed to download, such as your own backup or a publisher's download
          link. It's saved to your storage drive like any other download.
        </p>
        <AddDownloadForm bare />
      </section>
      <p className="muted small attribution">
        Game data from{" "}
        <button type="button" className="link-button" onClick={() => runAction(() => api.openExternal(game.rawgUrl))}>
          RAWG
        </button>
        .
      </p>
    </div>
  );
}

function Hero({ game }: { game: GameDetails }) {
  const released = game.released ? formatReleaseDate(game.released) : game.tba ? "To be announced" : null;
  return (
    <header className="game-hero">
      {game.image && <img className="game-hero-bg" src={game.image} alt="" />}
      <div className="game-hero-shade" />
      <div className="game-hero-content">
        <div className="chips">
          {game.genres.map((g) => (
            <span key={g} className="chip">
              {g}
            </span>
          ))}
        </div>
        <h1 className="game-title">{game.name}</h1>
        <div className="game-hero-stats">
          {game.metacritic !== null && (
            <span className="hero-stat">
              <Metascore score={game.metacritic} large /> Metascore
            </span>
          )}
          {game.rating > 0 && (
            <span className="hero-stat">
              <Icon name="star" size={16} className="star" />
              <strong>{game.rating.toFixed(1)}</strong>/{game.ratingTop || 5}
              <span className="hero-muted">({game.ratingsCount.toLocaleString()} ratings)</span>
            </span>
          )}
          {released && (
            <span className="hero-stat">
              <Icon name="history" size={16} /> {released}
            </span>
          )}
          {game.playtime > 0 && (
            <span className="hero-stat">
              <Icon name="clock" size={16} /> About {game.playtime} h to play
            </span>
          )}
        </div>
      </div>
    </header>
  );
}

function About({ game }: { game: GameDetails }) {
  const [expanded, setExpanded] = useState(false);
  const text = game.description || "RAWG doesn't have a description for this game yet.";
  const long = text.length > DESCRIPTION_PREVIEW;
  const shown = long && !expanded ? `${text.slice(0, DESCRIPTION_PREVIEW).trimEnd()}…` : text;
  return (
    <section className="card stack">
      <h2 className="card-title">About</h2>
      <div className="game-description">
        {shown.split(/\n+/).map((para, i) => (
          <p key={i}>{para}</p>
        ))}
      </div>
      {long && (
        <button type="button" className="link-button" onClick={() => setExpanded((e) => !e)} aria-expanded={expanded}>
          {expanded ? "Show less" : "Read more"}
        </button>
      )}
      {game.tags.length > 0 && (
        <div className="chips">
          {game.tags.map((t) => (
            <span key={t} className="chip chip-quiet">
              {t}
            </span>
          ))}
        </div>
      )}
    </section>
  );
}

function Media({ game }: { game: GameDetails }) {
  const [viewing, setViewing] = useState<number | null>(null);
  const shots = game.screenshots;
  if (!shots.length && !game.trailers.length) return null;
  return (
    <section className="card stack">
      <h2 className="card-title">Screenshots{game.trailers.length > 0 && " & trailers"}</h2>
      {game.trailers.length > 0 && (
        <div className="trailers">
          {game.trailers.slice(0, 2).map((t) => (
            <figure key={t.url} className="trailer">
              <video src={t.url} poster={t.preview ?? undefined} controls preload="none" />
              <figcaption className="muted small">{t.name}</figcaption>
            </figure>
          ))}
        </div>
      )}
      {shots.length > 0 && (
        <ul className="shots">
          {shots.map((src, i) => (
            <li key={src}>
              <button type="button" className="shot" onClick={() => setViewing(i)} aria-label={`View screenshot ${i + 1}`}>
                <img src={src} alt="" loading="lazy" />
              </button>
            </li>
          ))}
        </ul>
      )}
      {viewing !== null && <Lightbox images={shots} index={viewing} onChange={setViewing} onClose={() => setViewing(null)} />}
    </section>
  );
}

function Lightbox({ images, index, onChange, onClose }: { images: string[]; index: number; onChange: (i: number) => void; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  const step = useCallback((delta: number) => onChange((index + delta + images.length) % images.length), [index, images.length, onChange]);

  useEffect(() => {
    ref.current?.showModal();
  }, []);

  return (
    <dialog
      ref={ref}
      className="lightbox"
      onClose={onClose}
      onClick={(e) => e.target === e.currentTarget && ref.current?.close()}
      onKeyDown={(e) => {
        if (e.key === "ArrowRight") step(1);
        if (e.key === "ArrowLeft") step(-1);
      }}
    >
      <img src={images[index]} alt={`Screenshot ${index + 1} of ${images.length}`} />
      <div className="lightbox-bar">
        <Button size="sm" variant="secondary" onClick={() => step(-1)} aria-label="Previous screenshot">
          ‹
        </Button>
        <span className="small">
          {index + 1} / {images.length}
        </span>
        <Button size="sm" variant="secondary" onClick={() => step(1)} aria-label="Next screenshot">
          ›
        </Button>
        <Button size="sm" variant="secondary" icon="x" onClick={() => ref.current?.close()} aria-label="Close" autoFocus />
      </div>
    </dialog>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="fact">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Facts({ game }: { game: GameDetails }) {
  return (
    <section className="card stack">
      <h2 className="card-title">Details</h2>
      <dl className="facts">
        {game.platforms.length > 0 && (
          <Fact label="Platforms">
            <ul className="plain-list">
              {game.platforms.map((p) => (
                <li key={p.name}>
                  {p.name}
                  {p.releasedAt && p.releasedAt !== game.released && (
                    <span className="muted small"> · {formatReleaseDate(p.releasedAt)}</span>
                  )}
                </li>
              ))}
            </ul>
          </Fact>
        )}
        {game.released && <Fact label="Release date">{formatReleaseDate(game.released)}</Fact>}
        {game.developers.length > 0 && <Fact label="Developer">{game.developers.join(", ")}</Fact>}
        {game.publishers.length > 0 && <Fact label="Publisher">{game.publishers.join(", ")}</Fact>}
        {game.esrb && <Fact label="Age rating">ESRB {game.esrb}</Fact>}
        {game.stores.length > 0 && <Fact label="Available on">{game.stores.join(", ")}</Fact>}
        {game.alternativeNames.length > 0 && <Fact label="Also known as">{game.alternativeNames.join(", ")}</Fact>}
      </dl>
      {game.website && (
        <Button size="sm" icon="globe" onClick={() => runAction(() => api.openExternal(game.website!))}>
          Official website
        </Button>
      )}
    </section>
  );
}
