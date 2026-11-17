import { useState } from "react";
import type { AppStatus } from "../../shared/ipc";
import { call, errorText } from "../ipc";
import { ClientCard } from "../components/ClientCard";
import { ImportPanel } from "../components/ImportPanel";
import { MatchList } from "../components/MatchList";

interface Props {
  status: AppStatus | null;
  onRefresh: () => void;
}

export function HomePage({ status, onRefresh }: Props) {
  const [syncMsg, setSyncMsg] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);

  if (!status) return <p className="muted">Carregando…</p>;

  const syncData = () => {
    setSyncing(true);
    setSyncMsg(null);
    call("sync_ddragon", { force: true })
      .then((r) => setSyncMsg(`Dados do patch ${r.version} atualizados.`))
      .catch((e: unknown) => setSyncMsg(errorText(e)))
      .finally(() => {
        setSyncing(false);
        onRefresh();
      });
  };

  const c = status.counts;
  return (
    <div className="grid">
      <ClientCard lcu={status.lcu} />
      <section className="card">
        <h2>Dados</h2>
        <p>
          Conta: <strong>{status.userRiotId ?? "ainda não importada"}</strong>
        </p>
        <p>
          Partidas salvas: <strong>{c.matches}</strong>{" "}
          <span className="muted small">
            ({c.withTimeline} com linha do tempo, {c.remakes} remakes ignorados)
          </span>
        </p>
        <p>
          Data Dragon: <strong>{status.ddragonVersion ?? "não baixado"}</strong>{" "}
          <span className="muted small">
            ({c.champions} campeões, {c.items} itens, {c.runes} runas, {c.summonerSpells} feitiços)
          </span>
        </p>
        <button disabled={syncing} onClick={syncData}>
          {syncing ? "Atualizando…" : "Atualizar dados do jogo"}
        </button>
        {syncMsg && <p className="small">{syncMsg}</p>}
      </section>
      <ImportPanel status={status} />
      <MatchList ddragonVersion={status.ddragonVersion} reloadKey={c.matches + c.champions} />
    </div>
  );
}
