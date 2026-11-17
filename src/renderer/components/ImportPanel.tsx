import { useState } from "react";
import type { AppStatus } from "../../shared/ipc";
import { call, errorText } from "../ipc";
import { importEtaMinutes } from "../format";

export function ImportPanel({ status }: { status: AppStatus }) {
  const [error, setError] = useState<string | null>(null);
  const p = status.lastImport;
  const running = status.importing;
  const pct = p && p.total > 0 ? Math.round((p.done / p.total) * 100) : 0;
  const canStart = status.keyState === "ok" && !running;

  const start = () => {
    setError(null);
    call("start_import").catch((e: unknown) => setError(errorText(e)));
  };

  return (
    <section className="card wide">
      <h2>Importar partidas</h2>
      <p className="muted">
        Baixa suas últimas 100 ranqueadas Solo/Duo com a linha do tempo. Partidas já baixadas não são pedidas de
        novo. Com a chave de desenvolvimento, a primeira importação leva uns {importEtaMinutes(100)} minutos, e ela
        pausa sozinha se você entrar em partida.
      </p>
      <button className="primary" disabled={!canStart} onClick={start}>
        {running ? "Importando…" : "Importar últimas 100 ranqueadas"}
      </button>
      {p && (
        <div className="progress-block">
          {p.total > 0 && (
            <div className="progress" aria-label="Progresso da importação">
              <div className="bar" style={{ width: `${pct}%` }} />
            </div>
          )}
          <p>
            {p.step}
            {p.total > 0 && ` (${p.done}/${p.total}, ${p.newMatches} novas)`}
          </p>
          {p.error && <p className="error">{p.error}</p>}
        </div>
      )}
      {error && <p className="error">{error}</p>}
    </section>
  );
}
