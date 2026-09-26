/** Metacritic score chip, colored by Metacritic's own bands. */
export function Metascore({ score, large }: { score: number; large?: boolean }) {
  const tone = score >= 75 ? "good" : score >= 50 ? "mixed" : "bad";
  return (
    <span className={`metascore metascore-${tone}${large ? " metascore-lg" : ""}`} title="Metacritic score">
      {score}
    </span>
  );
}
