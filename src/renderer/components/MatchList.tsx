import { useEffect, useState } from "react";
import type { MatchSummary } from "../../shared/ipc";
import { call } from "../ipc";
import { csPerMinute, duration, kda, positionLabel, relativeDate } from "../format";

interface Props {
  ddragonVersion: string | null;
  /** Changes whenever the number of stored matches changes, to reload the list. */
  reloadKey: number;
}

export function MatchList({ ddragonVersion, reloadKey }: Props) {
  const [matches, setMatches] = useState<MatchSummary[]>([]);

  useEffect(() => {
    call("get_recent_matches", { limit: 20 })
      .then(setMatches)
      .catch(() => setMatches([]));
  }, [reloadKey]);

  if (matches.length === 0) {
    return (
      <section className="card wide">
        <h2>Últimas partidas</h2>
        <p className="muted">Nenhuma partida importada ainda.</p>
      </section>
    );
  }

  const wins = matches.filter((m) => m.win).length;
  return (
    <section className="card wide">
      <h2>
        Últimas partidas{" "}
        <span className="muted small">
          {wins}V {matches.length - wins}D nas últimas {matches.length} (remakes não contam)
        </span>
      </h2>
      <ul className="matches">
        {matches.map((m) => (
          <li key={m.matchId} className={m.win ? "match win" : "match loss"}>
            {ddragonVersion && m.championKey ? (
              <img
                src={`https://ddragon.leagueoflegends.com/cdn/${ddragonVersion}/img/champion/${m.championKey}.png`}
                alt={m.championName ?? ""}
                width={40}
                height={40}
              />
            ) : (
              <div className="img-placeholder" />
            )}
            <div className="match-main">
              <strong>{m.championName ?? `Campeão ${m.championId}`}</strong>
              <span className="muted small">
                {positionLabel(m.position)} · {relativeDate(m.gameStart)} · patch {m.patch}
              </span>
            </div>
            <div className="match-stats">
              <span>
                {m.kills}/{m.deaths}/{m.assists} <span className="muted">({kda(m.kills, m.deaths, m.assists)})</span>
              </span>
              <span className="muted small">
                {m.cs} CS ({csPerMinute(m.cs, m.durationS)}/min) · {duration(m.durationS)}
              </span>
            </div>
            <span className="result">{m.win ? "Vitória" : "Derrota"}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
