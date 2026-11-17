import { useEffect, useState } from "react";
import type { ConfigView } from "../../shared/ipc";
import { call, errorText } from "../ipc";

interface Props {
  onSaved: () => void;
}

type Feedback = { kind: "ok" | "error"; text: string } | null;

export function SettingsPage({ onSaved }: Props) {
  const [view, setView] = useState<ConfigView | null>(null);
  const [key, setKey] = useState("");
  const [riotId, setRiotId] = useState("");
  const [lolDir, setLolDir] = useState("");
  const [platform, setPlatform] = useState("br1");
  const [region, setRegion] = useState("americas");
  const [feedback, setFeedback] = useState<Feedback>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    call("get_config").then((v) => {
      setView(v);
      setRiotId(v.riotId ?? "");
      setLolDir(v.lolInstallDir ?? "");
      setPlatform(v.platform);
      setRegion(v.region);
    });
  }, []);

  const save = async (andTest: boolean) => {
    setBusy(true);
    setFeedback(null);
    try {
      const v = await call("save_config", {
        input: {
          riotApiKey: key.trim() || null,
          platform,
          region,
          lolInstallDir: lolDir.trim() || null,
          riotId: riotId.trim() || null,
        },
      });
      setView(v);
      setKey("");
      onSaved();
      if (andTest) {
        const msg = await call("test_key");
        setFeedback({ kind: "ok", text: msg });
      } else {
        setFeedback({ kind: "ok", text: "Configurações salvas." });
      }
    } catch (e) {
      setFeedback({ kind: "error", text: errorText(e) });
    } finally {
      setBusy(false);
      onSaved();
    }
  };

  if (!view) return <p className="muted">Carregando…</p>;

  return (
    <div className="settings">
      <section className="card wide">
        <h2>Chave da Riot</h2>
        <p className="muted">
          Gere em developer.riotgames.com e cole aqui. A chave fica só neste computador, no arquivo de configuração do
          Rota, e nunca aparece na tela depois de salva. A chave de desenvolvimento expira a cada 24 horas.
        </p>
        <p>
          Situação:{" "}
          <strong>
            {view.hasKey
              ? `configurada (termina em ${view.keyHint})${view.keyFromEnv ? ", vinda do .env" : ""}`
              : "não configurada"}
          </strong>
        </p>
        <label>
          Nova chave
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            placeholder="RGAPI-..."
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
      </section>

      <section className="card wide">
        <h2>Conta e cliente</h2>
        <label>
          Riot ID (usado quando o cliente do LoL está fechado)
          <input placeholder="Nome#TAG" value={riotId} onChange={(e) => setRiotId(e.target.value)} />
        </label>
        <label>
          Pasta do League of Legends (opcional)
          <input
            placeholder="C:\Riot Games\League of Legends"
            value={lolDir}
            onChange={(e) => setLolDir(e.target.value)}
          />
        </label>
        <div className="row">
          <label>
            Servidor
            <select value={platform} onChange={(e) => setPlatform(e.target.value)}>
              <option value="br1">Brasil (br1)</option>
              <option value="la1">LAN (la1)</option>
              <option value="la2">LAS (la2)</option>
              <option value="na1">NA (na1)</option>
            </select>
          </label>
          <label>
            Região das partidas
            <select value={region} onChange={(e) => setRegion(e.target.value)}>
              <option value="americas">americas</option>
            </select>
          </label>
        </div>
      </section>

      <div className="actions">
        <button className="primary" disabled={busy} onClick={() => save(true)}>
          Salvar e testar chave
        </button>
        <button disabled={busy} onClick={() => save(false)}>
          Só salvar
        </button>
      </div>
      {feedback && <p className={feedback.kind === "ok" ? "success" : "error"}>{feedback.text}</p>}
    </div>
  );
}
