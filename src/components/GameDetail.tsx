import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { formatReleaseDate } from "../lib/format";
import { runAction } from "../stores/app";
import { useGames } from "../stores/games";
import type { GameDetails, Page, Section } from "../types";
import { AddDownloadForm } from "./AddDownloadForm";
import { Button } from "./Button";
import { ErrorNotice } from "./ErrorNotice";
import { Icon } from "./Icon";
import { Metascore } from "./Metascore";

const openLink = (url: string) => runAction(() => api.openExternal(url));

export function GameDetail({ id, onNavigate }: { id: string; onNavigate: (page: Page) => void }) {
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
      <div className="page game-page">
        {back}
        <div className="game-hero skeleton" aria-busy="true" aria-label="Loading game details">
          <div className="game-hero-content">
            <div className="game-poster" />
            <div className="game-hero-text">
              <span className="skeleton-line wide" />
              <span className="skeleton-line" />
              <span className="skeleton-line short" />
            </div>
          </div>
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
          <Article sections={game.sections} />
          <Media game={game} />
        </div>
        <aside className="stack game-side">
          <Facts game={game} />
        </aside>
      </div>
      <section className="card stack game-download">
        <div className="card-head">
          <div className="card-title">
            <Icon name="download" size={18} />
            <h2>Download</h2>
          </div>
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
      <Attribution game={game} />
    </div>
  );
}

function Hero({ game }: { game: GameDetails }) {
  const background = game.backdrop ?? game.image;
  const released = formatReleaseDate(game.released);
  return (
    <header className="game-hero">
      {background && <img className={`game-hero-bg${game.backdrop ? "" : " is-blurred"}`} src={background} alt="" />}
      <div className="game-hero-shade" />
      <div className="game-hero-content">
        <div className="game-poster">
          {game.image ? <img src={game.image} alt={`${game.name} cover art`} /> : <Icon name="gamepad" size={40} />}
        </div>
        <div className="game-hero-text">
          {game.consoles.length > 0 && (
            <div className="console-badges static">
              {game.consoles.map((c) => (
                <span key={c} className="console-badge">
                  {c}
                </span>
              ))}
            </div>
          )}
          <h1 className="game-title">{game.name}</h1>
          {game.summary && <p className="game-summary">{capitalize(game.summary)}</p>}
          <div className="game-hero-stats">
            {game.criticScore && (
              <span className="hero-stat">
                <Metascore critic={game.criticScore} large /> {game.criticScore.source}
              </span>
            )}
            {game.rawg && (
              <span className="hero-stat">
                <Icon name="star" size={16} className="star" />
                <strong>{game.rawg.rating.toFixed(1)}</strong>/{game.rawg.ratingTop}
                <span className="hero-muted">({game.rawg.ratingsCount.toLocaleString()} ratings)</span>
              </span>
            )}
            {released && (
              <span className="hero-stat">
                <Icon name="history" size={16} /> {released}
              </span>
            )}
          </div>
          {game.genres.length > 0 && (
            <div className="chips">
              {game.genres.map((g) => (
                <span key={g} className="chip">
                  {capitalize(g)}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </header>
  );
}

function Article({ sections }: { sections: Section[] }) {
  const [active, setActive] = useState(0);
  const section = sections[active] ?? sections[0];
  if (!section) {
    return (
      <section className="card stack">
        <h2 className="card-title-text">About</h2>
        <p className="muted">Wikipedia doesn't have an article about this game yet.</p>
      </section>
    );
  }
  return (
    <section className="card article">
      {sections.length > 1 && (
        <div className="tabs" role="tablist" aria-label="Article sections">
          {sections.map((s, i) => (
            <button key={s.title} type="button" role="tab" aria-selected={i === active} onClick={() => setActive(i)}>
              {s.title}
            </button>
          ))}
        </div>
      )}
      <div className="article-body" role="tabpanel" aria-label={section.title}>
        {section.blocks.map((b, i) => (b.type === "heading" ? <h3 key={i}>{b.text}</h3> : <p key={i}>{b.text}</p>))}
      </div>
    </section>
  );
}

function Media({ game }: { game: GameDetails }) {
  const [viewing, setViewing] = useState<number | null>(null);
  const shots = game.screenshots;
  if (!shots.length && !game.trailers.length) return null;
  return (
    <section className="card stack">
      <h2 className="card-title-text">
        {game.trailers.length > 0 ? "Trailers & images" : "Images"}
        <span className="section-count">{shots.length + game.trailers.length}</span>
      </h2>
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
              <button type="button" className="shot" onClick={() => setViewing(i)} aria-label={`View image ${i + 1}`}>
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
      <img src={images[index]} alt={`Image ${index + 1} of ${images.length}`} />
      <div className="lightbox-bar">
        {images.length > 1 && (
          <Button size="sm" variant="secondary" onClick={() => step(-1)} aria-label="Previous image">
            ‹
          </Button>
        )}
        <span className="small">
          {index + 1} / {images.length}
        </span>
        {images.length > 1 && (
          <Button size="sm" variant="secondary" onClick={() => step(1)} aria-label="Next image">
            ›
          </Button>
        )}
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
      <h2 className="card-title-text">Details</h2>
      <dl className="facts">
        {game.platforms.length > 0 && (
          <Fact label="Platforms">
            <ul className="plain-list">
              {game.platforms.map((p) => (
                <li key={p.name}>
                  {p.name}
                  {p.releasedAt && <span className="muted small"> · {formatReleaseDate(p.releasedAt)}</span>}
                </li>
              ))}
            </ul>
          </Fact>
        )}
        {game.developers.length > 0 && <Fact label="Developer">{game.developers.join(", ")}</Fact>}
        {game.publishers.length > 0 && <Fact label="Publisher">{game.publishers.join(", ")}</Fact>}
        {game.series.length > 0 && <Fact label="Series">{game.series.join(", ")}</Fact>}
        {game.modes.length > 0 && <Fact label="Game modes">{game.modes.map(capitalize).join(", ")}</Fact>}
        {game.ageRatings.length > 0 && <Fact label="Age rating">{game.ageRatings.join(", ")}</Fact>}
        {game.alternativeNames.length > 0 && <Fact label="Also known as">{game.alternativeNames.join(", ")}</Fact>}
      </dl>
      <div className="fact-links">
        {game.wikipediaUrl && (
          <Button size="sm" icon="globe" onClick={() => openLink(game.wikipediaUrl!)}>
            Wikipedia
          </Button>
        )}
        {game.website && (
          <Button size="sm" icon="link" onClick={() => openLink(game.website!)}>
            Official site
          </Button>
        )}
      </div>
    </section>
  );
}

function Attribution({ game }: { game: GameDetails }) {
  return (
    <p className="muted small attribution">
      Text and images from{" "}
      {game.wikipediaUrl ? (
        <button type="button" className="link-button" onClick={() => openLink(game.wikipediaUrl!)}>
          Wikipedia
        </button>
      ) : (
        "Wikipedia"
      )}{" "}
      (CC BY-SA), facts from{" "}
      <button type="button" className="link-button" onClick={() => openLink(game.wikidataUrl)}>
        Wikidata
      </button>
      {game.rawg && (
        <>
          , ratings and media from{" "}
          <button type="button" className="link-button" onClick={() => openLink(game.rawg!.url)}>
            RAWG
          </button>
        </>
      )}
      .
    </p>
  );
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
