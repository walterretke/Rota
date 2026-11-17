import type { KeyState } from "../../shared/ipc";

interface Props {
  keyState: KeyState;
  onOpenSettings: () => void;
}

/** "Expired key" screen: shown whenever the Riot key is missing or was rejected (401/403). */
export function KeyBanner({ keyState, onOpenSettings }: Props) {
  if (keyState === "ok") return null;
  const expired = keyState === "invalid";
  return (
    <div className={expired ? "banner banner-error" : "banner banner-warn"} role="alert">
      <div>
        <strong>{expired ? "Sua chave da Riot expirou ou é inválida." : "Nenhuma chave da Riot configurada."}</strong>
        <p>
          {expired
            ? "Paramos todas as chamadas à Riot para não insistir com uma chave recusada. A chave de desenvolvimento vale 24 horas: gere outra em developer.riotgames.com e cole em Configurações."
            : "Gere sua chave em developer.riotgames.com e cole em Configurações. Ela fica só neste computador."}
        </p>
      </div>
      <button onClick={onOpenSettings}>Abrir Configurações</button>
    </div>
  );
}
