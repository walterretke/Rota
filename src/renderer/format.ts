// Display helpers (pt-BR).

const PHASES: Record<string, string> = {
  None: "No cliente",
  Lobby: "No lobby",
  Matchmaking: "Procurando partida",
  ReadyCheck: "Partida encontrada",
  ChampSelect: "Seleção de campeões",
  GameStart: "Carregando partida",
  InProgress: "Em partida",
  Reconnect: "Reconectando",
  WaitingForStats: "Aguardando estatísticas",
  PreEndOfGame: "Fim de jogo",
  EndOfGame: "Fim de jogo",
};

export function phaseLabel(phase: string | null): string {
  if (!phase) return "No cliente";
  return PHASES[phase] ?? phase;
}

const POSITIONS: Record<string, string> = {
  TOP: "Topo",
  JUNGLE: "Selva",
  MIDDLE: "Meio",
  BOTTOM: "Atirador",
  UTILITY: "Suporte",
};

export function positionLabel(position: string): string {
  return POSITIONS[position] ?? "—";
}

export function kda(kills: number, deaths: number, assists: number): string {
  if (deaths === 0) return "Perfeito";
  return ((kills + assists) / deaths).toFixed(2).replace(".", ",");
}

export function duration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function csPerMinute(cs: number, seconds: number): string {
  if (seconds <= 0) return "0,0";
  return (cs / (seconds / 60)).toFixed(1).replace(".", ",");
}

export function relativeDate(epochMs: number, now: number = Date.now()): string {
  const days = Math.floor((now - epochMs) / 86_400_000);
  if (days <= 0) return "hoje";
  if (days === 1) return "ontem";
  if (days < 30) return `há ${days} dias`;
  return new Date(epochMs).toLocaleDateString("pt-BR");
}

/** Sustained budget of the development key: 100 requests / 120 s, 2 per match (match + timeline). */
export function importEtaMinutes(remainingMatches: number): number {
  return Math.ceil((remainingMatches * 2 * 120) / 90 / 60);
}
