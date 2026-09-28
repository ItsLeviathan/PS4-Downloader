import type { CriticScore } from "../types";

/** Aggregate critic score chip, colored by Metacritic's bands. */
export function Metascore({ critic, large }: { critic: CriticScore; large?: boolean }) {
  const tone = critic.score >= 75 ? "good" : critic.score >= 50 ? "mixed" : "bad";
  return (
    <span className={`metascore metascore-${tone}${large ? " metascore-lg" : ""}`} title={`${critic.source} score`}>
      {critic.score}
    </span>
  );
}
