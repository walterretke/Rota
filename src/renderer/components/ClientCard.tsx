import type { LcuState } from "../../shared/ipc";
import { phaseLabel } from "../format";

export function ClientCard({ lcu }: { lcu: LcuState }) {
  return (
    <section className="card">
      <h2>Cliente do LoL</h2>
      {lcu.connected ? (
        <>
          <p className="status ok">● Conectado</p>
          <p>
            Fase: <strong>{phaseLabel(lcu.phase)}</strong>
          </p>
          <p>
            Conta:{" "}
            <strong>{lcu.user ? `${lcu.user.gameName}#${lcu.user.tagLine}` : "aguardando login"}</strong>
          </p>
        </>
      ) : (
        <>
          <p className="status off">● Fechado</p>
          <p className="muted">Abra o cliente do LoL. O Rota detecta sozinho em alguns segundos.</p>
        </>
      )}
    </section>
  );
}
