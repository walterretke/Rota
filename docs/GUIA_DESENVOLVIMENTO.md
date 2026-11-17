# Guia de Desenvolvimento — App de Análise Ranqueada (League of Legends)

> Documento-base do projeto. Toda funcionalidade nova deve ser checada contra a **Seção 1 (Conformidade)** antes de ser implementada.
> Região principal: **Brasil (BR1)**. Fila principal: **Ranqueada Solo/Duo (queueId 420)**.

---

## Sumário

0. [Objetivo do produto](#0-objetivo-do-produto)
1. [Conformidade com a Riot — linhas vermelhas](#1-conformidade-com-a-riot--linhas-vermelhas)
2. [Chave de API e segurança](#2-chave-de-api-e-segurança)
3. [Rate limits e roteamento](#3-rate-limits-e-roteamento)
4. [Mapa das APIs (o que cada uma faz e como usamos)](#4-mapa-das-apis)
5. [Dados estáticos — Data Dragon](#5-dados-estáticos--data-dragon)
6. [Live Client Data API (dados locais durante a partida)](#6-live-client-data-api)
   - [6.5 LCU API (cliente do LoL)](#65-lcu-api-cliente-do-lol)
7. [Módulos / funcionalidades do app](#7-módulos--funcionalidades)
8. [Catálogo de métricas (fórmulas e campos)](#8-catálogo-de-métricas)
9. [Estatística: como não se enganar com os números](#9-estatística)
10. [Arquitetura e stack](#10-arquitetura-e-stack)
11. [Modelo de dados](#11-modelo-de-dados)
12. [Crawler do meta do elo](#12-crawler-do-meta-do-elo)
13. [Roadmap por fases](#13-roadmap-por-fases)
14. [Pegadinhas conhecidas](#14-pegadinhas-conhecidas)
15. [Referências](#15-referências)

---

## 0. Objetivo do produto

Dar ao jogador **vantagem legítima** no ranqueado através de:

- **Autoconhecimento**: onde ele ganha e perde jogos (pós-jogo e tendências).
- **Contexto do elo**: o meta real do *seu* elo no BR, não a média global.
- **Decisões melhores**: escolha de campeão, runas e itens apresentados como **opções com justificativa**, nunca como ordens.

O que o app **não** é: um cheat, um overlay de timers, um "rastreador de inimigo", uma calculadora de MMR, nem um "dodge advisor".

**Custo zero de operação:** o app **não usa IA paga / LLM**. Toda análise é feita por estatística e regras determinísticas (Seções 8 e 9), rodando localmente. Riot API, Data Dragon, LCU e Live Client Data são gratuitas. Um resumo em linguagem natural via LLM fica como extra **opcional** e desligado por padrão (de preferência com modelo local, ex.: Ollama).

**Formato:** aplicativo **desktop para Windows** (no estilo Blitz / U.GG / OP.GG Desktop), que roda ao lado do cliente do LoL e reage sozinho a cada fase: cliente aberto → seleção de campeões → partida → fim de jogo. Usa três fontes:

| Fonte | Onde roda | Para quê |
|---|---|---|
| **Riot API** (Seção 4) | Internet, com chave | Histórico, elo, partidas, meta |
| **LCU API** (Seção 6.5) | Local, cliente do LoL | Detectar a fase, ler a seleção de campeões, importar runas/feitiços/itens |
| **Live Client Data API** (Seção 6) | Local, durante a partida | Itens e números ao vivo |

---

## 1. Conformidade com a Riot — linhas vermelhas

Esta seção é obrigatória. Se uma feature cair em "Proibido", ela não entra. Se cair em "Zona cinza", não entra sem aprovação explícita da Riot.

### 1.1 Permitido / incentivado

| Feature | Observação |
|---|---|
| Estatísticas do próprio jogador (histórico, win rate, KDA, CS etc.) | Núcleo do app |
| Análise pós-jogo com timeline | Núcleo do app |
| Meta agregado (win rate de campeões, matchups, builds por elo) | Com dados da API oficial |
| Sugestões de build, runas e ordem de skills | Apresentar **várias opções** com justificativa |
| Sugestão de campeão na seleção (baseada no próprio pool + rota inimiga) | Apresentar **2–3 opções**, não "pegue X" |
| Consulta de partida ao vivo após o carregamento (spectator) — elo e campeões dos jogadores | Padrão de mercado (estilo tela de carregamento); dados públicos |
| Sugestão de itens durante o jogo com base nos itens visíveis do inimigo | Informação já visível pelo Tab |
| Desafios, maestrias, Clash | Dados oficiais |

### 1.2 Proibido

| Feature | Motivo |
|---|---|
| Timers de ultimate (inimigo ou aliado), timers de Flash/feitiços inimigos | Proibido explicitamente |
| Alertas de "power spike" (ex.: inimigo chegou ao nível 6) | Proibido explicitamente |
| Timers de campos da selva / rastreamento de jungler inimigo | Altera "campo de informação"; tratar como proibido |
| Calculadora/estimativa de MMR ou ELO, ladder alternativo | Proibido explicitamente pela política |
| Revelar nomes/históricos dos aliados na seleção de campeões da Solo/Duo | Anonimato da seleção de campeões; mostrar como "Aliado 1..4" |
| "Sugestão de dodge" baseada em quem são os aliados | Decorre do item acima |
| Identificar jogadores ocultos/anônimos por cruzamento de dados | Proibido explicitamente |
| Ler memória do jogo, injetar código, automatizar ações | Cheat |
| Qualquer coisa que **decida pelo jogador** (auto-pick, "faça isso agora") | "Apps que ditam decisões" são vetados |
| Apostas, cripto/blockchain, anúncios dentro do jogo/cliente | Proibido pela política geral |

### 1.3 Zona cinza (não implementar sem aprovação)

- Timers de objetivos neutros (dragão/barão) no overlay.
- Qualquer overlay in-game além de "itens sugeridos" e "meus próprios números".
- Qualquer **ação automática no cliente** além de importar runas/feitiços/conjunto de itens (ex.: aceitar fila, escolher ou banir campeão sozinho) → **não fazer**.

### 1.3.1 LCU API — usamos, mas com regras

A LCU (API local do cliente) é o que permite a experiência "Blitz/U.GG". Ela **não é oficialmente suportada** pela Riot para terceiros (sem garantia de documentação, estabilidade ou aviso de mudanças), e a Riot pede que o uso seja **registrado no Developer Portal**. Regras do projeto:

- ✅ Ler a fase do jogo (`gameflow`) e os **campeões / rotas** da seleção.
- ✅ Importar runas, feitiços e conjunto de itens **para o próprio usuário** (ao travar o campeão ou com um clique).
- ❌ Ler ou tentar resolver **identidade dos aliados** na Solo/Duo (nome, PUUID, histórico). Na seleção, usar só `championId`, `assignedPosition` e `cellId`.
- ❌ Auto-aceitar fila, auto-pick, auto-ban, mensagens automáticas no chat.
- ❌ Mods/injeção no cliente (ex.: "loaders" que alteram a interface do cliente).

### 1.4 Regras gerais de produto (se for público/monetizado)

- Produto **registrado** no Developer Portal com status *Approved* ou *Acknowledged*.
- **Tier gratuito obrigatório** (pode ter anúncios fora do jogo).
- Conteúdo pago precisa ser **transformativo** (agregar análise, não só repassar dados).
- Uma chave de produção **por produto**.
- Exibir **Riot ID** (`gameName#tagLine`), nunca o antigo nome de invocador.
- Usar apenas o **press kit** como asset de IP da Riot no marketing.

### 1.5 Regra de ouro para UX

> Toda recomendação deve ter o formato **"Opções + motivo + dado"**.
> Ex.: *"Contra Darius, do seu pool: **Vayne** (55% de WR no seu elo, n=812, kite), **Quinn** (53%, n=340), **Teemo** (52%). Você tem 61% com Vayne nos últimos 30 jogos."*

---

## 2. Chave de API e segurança

### 2.1 Tipos de chave

| Tipo | Uso | Expira? | Limites |
|---|---|---|---|
| **Development** | Prototipar. É a que temos agora. | **Sim, a cada 24h** — regenerar no portal | 20 req/1s e 100 req/2min **por routing value** |
| **Personal** | Projeto pessoal, mostrar as próprias estatísticas | Não | Mesmos limites da development (confirmar no portal) |
| **Production** | App público | Não | Bem maiores; exige aprovação do produto |

**Ação imediata:** a chave atual expirou (16/11/2026, 12:23 PT). Regenerar no portal e, assim que o MVP pessoal estiver de pé, **solicitar uma Personal Key** (registrar o produto) para parar de regenerar todo dia.

### 2.2 Regras de segurança (obrigatórias)

- A chave vai em **variável de ambiente** (`RIOT_API_KEY`) — nunca no código, nunca no Git, nunca no front-end, nunca em binário distribuído.
- `.env` no `.gitignore`. Manter um `.env.example` sem valor.
- **No app desktop:** a chave fica num arquivo de configuração na pasta de dados do usuário (ex.: `%APPDATA%\<app>\config.json`), digitada pela tela de configurações — nunca empacotada no instalador. Se o app for distribuído para outras pessoas, a chave sai do app e vai para um servidor nosso (Seção 10.1).
- Todas as chamadas via **HTTPS**, header `X-Riot-Token: <chave>`.
- O front-end **nunca** chama a Riot direto: sempre passa pelo nosso back-end (que guarda a chave e controla o rate limit).
- Ao receber `401/403`: a chave expirou ou é inválida → alertar no log e pausar a fila (não ficar martelando).

```env
# .env.example
RIOT_API_KEY=
RIOT_PLATFORM=br1
RIOT_REGION=americas
DATABASE_URL=postgresql://user:pass@localhost:5432/lolapp
```

---

## 3. Rate limits e roteamento

### 3.1 Routing values

| Tipo | Valor BR | Host | Usado por |
|---|---|---|---|
| **Plataforma** | `br1` | `https://br1.api.riotgames.com` | summoner-v4, league-v4, league-exp-v4, champion-mastery-v4, spectator-v5, champion-v3, clash-v1, lol-challenges-v1, lol-status-v4 |
| **Regional** | `americas` | `https://americas.api.riotgames.com` | account-v1, match-v5 |

**Importante:** os limites são **por routing value**. Ou seja, temos dois "baldes" independentes:
- Balde `americas`: 20/s e 100/2min → usado por **partidas** (o gargalo).
- Balde `br1`: 20/s e 100/2min → usado por elo, maestria, spectator etc.

Além do limite da aplicação, cada endpoint tem um **limite por método**. Ler sempre os headers.

### 3.2 Headers a ler em toda resposta

| Header | Significado |
|---|---|
| `X-App-Rate-Limit` | Ex.: `20:1,100:120` (limite:janela_em_segundos) |
| `X-App-Rate-Limit-Count` | Uso atual em cada janela |
| `X-Method-Rate-Limit` / `-Count` | Mesmo, por endpoint |
| `Retry-After` | Em um `429`, segundos para esperar |

### 3.3 Orçamento com a chave de desenvolvimento

O limite efetivo sustentado é **100 req / 120s ≈ 0,83 req/s por balde**.

| Tarefa | Requisições (balde americas) | Tempo aproximado |
|---|---|---|
| Lista de IDs das últimas 100 ranqueadas | 1 | instantâneo |
| Detalhes de 1 partida + timeline | 2 | — |
| Importar 100 partidas do jogador com timeline | ~201 | ~4 min |
| Crawler de meta (partida + timeline) | 2 por partida | **~1.500 partidas/hora** |

Conclusões de projeto:
- **Cache agressivo**: uma partida finalizada nunca muda. Baixou uma vez, nunca mais pede.
- **Timeline sob demanda** no crawler de meta (só baixar se a feature precisar dela).
- **Fila com prioridade**: requisições do usuário (ex.: "atualizar meu perfil") passam na frente do crawler.

### 3.4 Cliente HTTP com rate limit (referência em Python)

```python
# riot_client.py — esqueleto de referência
import os, time, asyncio, collections
import httpx

class SlidingWindowLimiter:
    """Respeita múltiplas janelas (ex.: 20/1s e 100/120s) para UM routing value."""
    def __init__(self, limits=((20, 1), (100, 120)), safety=0.9):
        self.limits = [(int(n * safety), w) for n, w in limits]
        self.calls = collections.deque()
        self.lock = asyncio.Lock()

    async def acquire(self):
        async with self.lock:
            while True:
                now = time.monotonic()
                max_window = max(w for _, w in self.limits)
                while self.calls and now - self.calls[0] > max_window:
                    self.calls.popleft()
                wait = 0.0
                for n, w in self.limits:
                    recent = [t for t in self.calls if now - t <= w]
                    if len(recent) >= n:
                        wait = max(wait, w - (now - recent[0]) + 0.05)
                if wait <= 0:
                    self.calls.append(now)
                    return
                await asyncio.sleep(wait)

class RiotClient:
    def __init__(self):
        self.key = os.environ["RIOT_API_KEY"]
        self.platform = os.getenv("RIOT_PLATFORM", "br1")
        self.region = os.getenv("RIOT_REGION", "americas")
        self.limiters = {self.platform: SlidingWindowLimiter(),
                         self.region: SlidingWindowLimiter()}
        self.http = httpx.AsyncClient(timeout=15,
                                      headers={"X-Riot-Token": self.key})

    async def get(self, routing: str, path: str, params=None, retries=3):
        url = f"https://{routing}.api.riotgames.com{path}"
        for attempt in range(retries):
            await self.limiters[routing].acquire()
            r = await self.http.get(url, params=params)
            if r.status_code == 200:
                return r.json()
            if r.status_code == 404:
                return None
            if r.status_code == 429:
                await asyncio.sleep(int(r.headers.get("Retry-After", 5)))
                continue
            if r.status_code in (401, 403):
                raise RuntimeError("Chave inválida/expirada — regenerar no portal")
            if r.status_code >= 500:
                await asyncio.sleep(2 ** attempt)
                continue
            r.raise_for_status()
        raise RuntimeError(f"Falhou após {retries} tentativas: {url}")
```

> Alternativa: usar uma lib pronta que já faz rate limit (ex.: **Cassiopeia** / **Pyot** em Python, **twisted** / **galeforce** em Node). Avaliar antes de reinventar.

---

## 4. Mapa das APIs

Legenda de routing: **[R]** = regional (`americas`), **[P]** = plataforma (`br1`).

### 4.1 account-v1 — identidade do jogador **[R]**

| Endpoint | Retorna |
|---|---|
| `GET /riot/account/v1/accounts/by-riot-id/{gameName}/{tagLine}` | `puuid`, `gameName`, `tagLine` |
| `GET /riot/account/v1/accounts/by-puuid/{puuid}` | Riot ID atual de um PUUID |

**Como usamos:** porta de entrada. O usuário digita `Nome#BR1` → obtemos o **PUUID**, que é a chave de tudo no banco.
**Cache:** PUUID é permanente. Riot ID pode mudar → revalidar a cada 7 dias.

### 4.2 summoner-v4 — conta de LoL **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/summoner/v4/summoners/by-puuid/{puuid}` | nível, ícone, `revisionDate` |

**Como usamos:** ícone e nível no perfil; `revisionDate` indica se houve atividade nova (evita requisições inúteis).
**Cache:** 1h.

### 4.3 league-v4 — elo **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/league/v4/entries/by-puuid/{puuid}` | tier, divisão, PDL, vitórias, derrotas, `hotStreak` por fila |
| `GET /lol/league/v4/entries/{queue}/{tier}/{division}?page=N` | Lista paginada de jogadores de um elo (Ferro–Diamante) |
| `GET /lol/league/v4/challengerleagues/by-queue/{queue}` | Toda a liga Desafiante |
| `GET /lol/league/v4/grandmasterleagues/by-queue/{queue}` | Grão-Mestre |
| `GET /lol/league/v4/masterleagues/by-queue/{queue}` | Mestre |

`queue` = `RANKED_SOLO_5x5` ou `RANKED_FLEX_SR`.

**Como usamos:**
- Card de elo do usuário e **histórico de PDL** (gravamos um snapshot a cada atualização → gráfico de evolução, que a API não dá pronto).
- **Semente do crawler**: listar jogadores de um elo para coletar partidas e montar o meta daquele elo.
- Elo médio da partida (para contextualizar análises).

⚠️ Não transformar isso em "estimativa de MMR". Mostramos só o elo oficial.

### 4.4 league-exp-v4 — elo (experimental) **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/league-exp/v4/entries/{queue}/{tier}/{division}?page=N` | Igual ao anterior, mas inclui Mestre+ paginado |

**Como usamos:** alternativa para semear o crawler de forma uniforme em todos os tiers.

### 4.5 match-v5 — partidas **[R]** (a API mais importante)

| Endpoint | Retorna |
|---|---|
| `GET /lol/match/v5/matches/by-puuid/{puuid}/ids` | Lista de IDs (`BR1_123...`). Params: `queue=420`, `type=ranked`, `start`, `count` (máx. 100), `startTime`, `endTime` (epoch em segundos) |
| `GET /lol/match/v5/matches/{matchId}` | Resultado completo: 10 participantes, campeões, itens, runas, KDA, ouro, dano, visão, objetivos, `challenges` (dezenas de métricas prontas) |
| `GET /lol/match/v5/matches/{matchId}/timeline` | **Frames a cada 60s** por participante (posição x/y, ouro, XP, nível, CS, stats) + **eventos** com timestamp (kills com posição, wards, compras de item, level de skill, objetivos, torres, placas) |

**Campos-chave de `info.participants[]`:**
`puuid`, `riotIdGameName`, `riotIdTagline`, `championId`, `championName`, `teamPosition` (TOP/JUNGLE/MIDDLE/BOTTOM/UTILITY), `win`, `kills`, `deaths`, `assists`, `totalMinionsKilled`, `neutralMinionsKilled`, `goldEarned`, `totalDamageDealtToChampions`, `damageSelfMitigated`, `visionScore`, `wardsPlaced`, `wardsKilled`, `detectorWardsPlaced`, `item0..item6`, `summoner1Id`, `summoner2Id`, `perks` (runas), `champLevel`, `timePlayed`, `gameEndedInEarlySurrender`, `challenges.*`.

**Objeto `challenges` (métricas prontas úteis):** `killParticipation`, `goldPerMinute`, `damagePerMinute`, `teamDamagePercentage`, `visionScorePerMinute`, `laneMinionsFirst10Minutes`, `jungleCsBefore10Minutes`, `maxCsAdvantageOnLaneOpponent`, `maxLevelLeadLaneOpponent`, `soloKills`, `skillshotsHit`, `skillshotsDodged`, `turretPlatesTaken`, `controlWardsPlaced`, `earlyLaningPhaseGoldExpAdvantage`, `laningPhaseGoldExpAdvantage`, entre outros. (Os nomes podem mudar entre patches — tratar como opcionais.)

**Eventos da timeline mais úteis:**
`CHAMPION_KILL` (com `position`, `killerId`, `victimId`, `assistingParticipantIds`), `WARD_PLACED`, `WARD_KILL`, `ITEM_PURCHASED`, `ITEM_SOLD`, `ITEM_UNDO`, `SKILL_LEVEL_UP`, `LEVEL_UP`, `ELITE_MONSTER_KILL` (dragão, arauto, barão, vastilarvas, Atakhan), `BUILDING_KILL`, `TURRET_PLATE_DESTROYED`, `CHAMPION_SPECIAL_KILL` (first blood, multikill), `GAME_END`.

**Como usamos:** praticamente todos os módulos (Seção 7).
**Cache:** permanente (partida finalizada é imutável).

### 4.6 champion-mastery-v4 — maestria **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}` | Maestria em todos os campeões (pontos, nível, última vez jogado) |
| `GET /lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/top?count=N` | Top N |
| `GET /lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/by-champion/{championId}` | Um campeão |
| `GET /lol/champion-mastery/v4/scores/by-puuid/{puuid}` | Score total |

**Como usamos:** contexto do pool do usuário ("você tem 400k pontos de Vayne mas só jogou 3 ranqueadas com ela nesta temporada"); na tela de carregamento, mostrar se o inimigo é "main" do campeão (via spectator, depois do jogo iniciar).
**Cache:** 6h.

### 4.7 spectator-v5 — partida ao vivo **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/spectator/v5/active-games/by-summoner/{puuid}` | Partida em andamento: 10 jogadores (PUUID, Riot ID), campeões, feitiços, runas, bans, fila, horário de início |
| `GET /lol/spectator/v5/featured-games` | Partidas em destaque |

**Como usamos:** **Módulo de Tela de Carregamento** (7.9): quando o usuário entra no jogo, mostrar elo, campeão principal e maestria dos 10 jogadores + dicas do matchup.
⚠️ Só depois que a partida começou. Nunca tentar descobrir jogadores durante a seleção de campeões.

### 4.8 champion-v3 — rotação **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/platform/v3/champion-rotations` | Campeões grátis da semana (inclusive para contas novas) |

**Como usamos:** baixa prioridade; sugerir campeões grátis para testar antes de comprar.

### 4.9 lol-challenges-v1 — desafios **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/challenges/v1/player-data/{puuid}` | Progresso do jogador em todos os desafios |
| `GET /lol/challenges/v1/challenges/config` | Definição dos desafios |
| `GET /lol/challenges/v1/challenges/percentiles` | Distribuição de todos os jogadores |

**Como usamos:** benchmark de habilidades (ex.: percentil de visão, de solo kills) e gamificação.

### 4.10 clash-v1 — Clash **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/clash/v1/players/by-puuid/{puuid}` | Inscrição do jogador no Clash, time, posição |
| `GET /lol/clash/v1/teams/{teamId}` | Membros do time |
| `GET /lol/clash/v1/tournaments` | Torneios ativos/futuros |

**Como usamos:** módulo de preparação para Clash (7.12) — estudar o time adversário do chaveamento (dados públicos do torneio).

### 4.11 lol-status-v4 — status do servidor **[P]**

| Endpoint | Retorna |
|---|---|
| `GET /lol/status/v4/platform-data` | Manutenções e incidentes |

**Como usamos:** banner "servidor BR com instabilidade" e pausar o crawler em manutenção.

### 4.12 Fora do escopo inicial

- **tournament-v5 / tournament-stub-v5**: só se um dia organizarmos torneios (exige chave de produção e regras próprias).
- **lol-rso-match-v1 / RSO (login com conta Riot)**: exige chave de produção. Útil no futuro para o usuário "provar" que é dono da conta.

---

## 5. Dados estáticos — Data Dragon

CDN oficial com dados e imagens por patch. **Não conta no rate limit.**

| Recurso | URL |
|---|---|
| Versões (a 1ª é a atual) | `https://ddragon.leagueoflegends.com/api/versions.json` |
| Campeões (resumo) | `https://ddragon.leagueoflegends.com/cdn/{versao}/data/pt_BR/champion.json` |
| Campeões (completo: skills, dicas, tags) | `.../data/pt_BR/championFull.json` |
| Itens | `.../data/pt_BR/item.json` |
| Runas | `.../data/pt_BR/runesReforged.json` |
| Feitiços de invocador | `.../data/pt_BR/summoner.json` |
| Ícone do campeão | `https://ddragon.leagueoflegends.com/cdn/{versao}/img/champion/{id}.png` |
| Ícone do item | `.../cdn/{versao}/img/item/{itemId}.png` |
| Splash | `https://ddragon.leagueoflegends.com/cdn/img/champion/splash/{id}_0.jpg` |
| Pacote completo | `https://ddragon.leagueoflegends.com/cdn/dragontail-{versao}.tgz` |

**Como usamos:**
- Traduzir IDs → nomes/ícones (o `key` de `champion.json` é o `championId` numérico da match-v5).
- **Tags e atributos** dos campeões (Tank, Mage, Assassin…) para análise de composição.
- Classificar itens (dano físico, mágico, resistências, anti-cura) para a sugestão de itens.
- Job diário: checar `versions.json`; se mudou, baixar e atualizar a tabela estática.

> Atenção: o Data Dragon às vezes atrasa após um patch. A **CommunityDragon** (`raw.communitydragon.org`) é uma alternativa não oficial com dados mais detalhados — usar só como complemento.

---

## 6. Live Client Data API

API **local**, servida pelo próprio jogo em `https://127.0.0.1:2999/liveclientdata/` **somente durante uma partida**. Não usa chave nem rate limit. Certificado autoassinado (usar o `riotgames.pem` da documentação ou desabilitar a verificação só para localhost).

| Endpoint | Retorna |
|---|---|
| `/allgamedata` | Tudo (pesado — evitar polling frequente) |
| `/activeplayer` | Seus status completos (ouro atual, atributos, runas, habilidades) |
| `/activeplayername` | Seu Riot ID |
| `/playerlist` | Os 10 jogadores: campeão, itens, nível, placar, time, posição |
| `/playeritems?riotId=` | Itens de um jogador |
| `/playerscores?riotId=` | KDA e CS de um jogador |
| `/eventdata` | Eventos da partida até agora |
| `/gamestats` | Tempo de jogo, modo, mapa |

**Como usamos (apenas o permitido):**
- Sugestão de **próximo item** com base no ouro atual do usuário e nos **itens visíveis** do time inimigo (ex.: inimigo com 3 itens de cura → opções de anti-cura).
- Painel do **próprio desempenho** (CS/min ao vivo vs. meta do elo).
- Registrar a partida localmente para enriquecer a análise pós-jogo.

**Nunca usar para:** timers de feitiços/ultimate inimigos, alertas de nível 6, rastreamento de jungler, timers de campos.

**Polling:** a cada ~2s em `/activeplayer` + `/playerlist` + `/eventdata` (endpoints locais, sem rate limit). Evitar `/allgamedata` em loop. Insights detalhados na Seção 7.10.

> A Live Client Data é a **única** fonte de dados ao vivo permitida. Desde 6/10/2026 a Riot bloqueia por padrão o acesso à memória do jogo por apps desconhecidos, e apps aprovados têm até abril/2027 para migrar para APIs oficiais.

---

## 6.5 LCU API (cliente do LoL)

API REST + WebSocket servida pelo **cliente** do LoL (não pelo jogo) enquanto ele está aberto. Não usa chave da Riot nem rate limit. Ver regras na Seção 1.3.1.

### 6.5.1 Conexão

1. Localizar o **lockfile** na pasta de instalação (padrão: `C:\Riot Games\League of Legends\lockfile`). Alternativa: ler os argumentos do processo `LeagueClientUx.exe` (`--app-port` e `--remoting-auth-token`).
2. Conteúdo: `LeagueClient:<pid>:<porta>:<senha>:https`.
3. REST: `https://127.0.0.1:<porta>/...` com **Basic Auth** usuário `riot`, senha `<senha>`. Certificado autoassinado (aceitar só para 127.0.0.1).
4. WebSocket: `wss://riot:<senha>@127.0.0.1:<porta>/` e enviar `[5, "OnJsonApiEvent"]` para assinar todos os eventos (protocolo WAMP). Assim o app **reage a eventos** em vez de ficar fazendo polling.
5. O lockfile some quando o cliente fecha e muda a cada abertura → observar o arquivo e reconectar.

> Bibliotecas prontas: **lcu-driver** (Python), **league-connect** (Node), **GlassLCU** (C#). Para explorar os endpoints: **Rift Explorer** / **Needlework**.

### 6.5.2 Endpoints que usamos

| Endpoint | Uso |
|---|---|
| `GET /lol-gameflow/v1/gameflow-phase` | Fase atual: `None`, `Lobby`, `Matchmaking`, `ReadyCheck`, `ChampSelect`, `InProgress`, `WaitingForStats`, `PreEndOfGame`, `EndOfGame` |
| `GET /lol-summoner/v1/current-summoner` | Quem é o usuário logado (PUUID, Riot ID) → evita o usuário ter que digitar o Riot ID |
| `GET /lol-champ-select/v1/session` | Estado da seleção: `myTeam`, `theirTeam` (campeões), `bans`, `actions`, `localPlayerCellId`, `assignedPosition` |
| `PATCH /lol-champ-select/v1/session/my-selection` | Definir feitiços (`spell1Id`, `spell2Id`) do usuário |
| `GET/POST/DELETE /lol-perks/v1/pages` | Ler, criar e apagar páginas de runas (importação de runas) |
| `GET /lol-perks/v1/inventory` | Quantas páginas editáveis o usuário tem (para saber se precisa substituir uma) |
| `PUT /lol-item-sets/v1/item-sets/{summonerId}/sets` | Gravar conjunto de itens recomendado (aparece na loja dentro do jogo) |
| `GET /lol-end-of-game/v1/eog-stats-block` | Resultado assim que a partida acaba → disparar a análise pós-jogo |

> Os endpoints podem mudar a qualquer patch, sem aviso. Encapsular todos num único módulo `lcu/` e tratar falhas sem derrubar o app.

### 6.5.3 Máquina de estados do app

```text
Cliente fechado ──(lockfile aparece)──► Conectado
Conectado ──(gameflow = ChampSelect)──► Seleção de campeões
   • lê /session a cada evento
   • mostra opções de pick/ban (Módulo 7.7) só com campeões e rotas
   • ao travar o campeão → importa runas/feitiços/itens (configurável)
Seleção ──(gameflow = InProgress)──► Em partida
   • Live Client Data (itens, números próprios)
   • spectator-v5 → tela de carregamento (Módulo 7.9)
Em partida ──(gameflow = EndOfGame)──► Pós-jogo
   • aguarda a partida aparecer na match-v5 (pode levar alguns minutos — tentar com backoff)
   • baixa partida + timeline → abre a análise (Módulo 7.2)
```

---

## 7. Módulos / funcionalidades

Cada módulo tem: **objetivo → dados → lógica → saída → conformidade → prioridade**.
Prioridade: **P0** (MVP), **P1** (segunda leva), **P2** (futuro).

### 7.1 Perfil e histórico — P0

- **Objetivo:** visão geral do jogador.
- **Dados:** account-v1, summoner-v4, league-v4, match-v5 (ids + detalhes).
- **Lógica:**
  1. Riot ID → PUUID.
  2. Elo atual (gravar snapshot de PDL com timestamp).
  3. Baixar as últimas N ranqueadas (`queue=420`) que ainda não estão no banco.
  4. Agregar por campeão, rota, dia da semana e hora.
- **Saída:** card de elo, gráfico de PDL ao longo do tempo (snapshots nossos), tabela por campeão (jogos, WR, KDA, CS/min, dano/min), WR por rota.
- **Conformidade:** ✅

### 7.2 Análise pós-jogo (timeline) — P0 ⭐ maior valor

- **Objetivo:** explicar **por que** a partida foi ganha ou perdida.
- **Dados:** match-v5 detalhes + timeline.
- **Lógica:**
  - Identificar o **oponente de rota** (mesmo `teamPosition`, time oposto).
  - Curvas minuto a minuto: diferença de ouro, XP e CS contra o oponente.
  - Snapshots @10 e @15: CS, ouro, XP, nível (ver métricas na Seção 8).
  - **Mortes**: minuto, posição (x/y), quem matou, se havia ward aliada próxima, se foi antes de um objetivo.
  - **Visão**: wards por minuto, ward de controle, onde foram colocadas.
  - **Objetivos**: participação em dragões/arauto/barão/vastilarvas (estava a menos de X unidades no momento do evento?).
  - **Itens**: ordem e minuto de cada item finalizado vs. média do elo para o campeão.
  - **Skills**: ordem de upgrades vs. a mais comum no meta.
- **Saída:**
  - Gráfico de diferença de ouro (time e rota).
  - **Mapa de Summoner's Rift** com pontos das mortes (coordenadas ~0–15000 nos dois eixos).
  - Linha do tempo com eventos-chave ("14:32 morreu sem visão no rio inimigo 40s antes do dragão").
  - 3 pontos positivos + 3 pontos de melhoria, gerados por regras (Seção 8.3).
- **Conformidade:** ✅ (análise de partida finalizada)

### 7.3 Coach de hábitos / relatório semanal — P1

- **Objetivo:** padrões recorrentes em várias partidas, não em uma só.
- **Dados:** banco local (partidas já baixadas).
- **Lógica:**
  - Heatmap agregado de mortes (últimos 20–50 jogos).
  - % de mortes antes dos 15 min, sem visão, sozinho (sem aliados por perto).
  - CS@10 médio vs. média do elo para os mesmos campeões.
  - Correlação métricas × vitória para o **próprio** jogador (ex.: "quando seu CS@10 ≥ 70, você ganha 62%; abaixo, 41%").
  - Tendência: métricas melhorando ou piorando nas últimas semanas.
- **Saída:** relatório semanal com os **3 hábitos que mais custam vitórias** + 1 meta concreta para a semana.
- **Conformidade:** ✅

### 7.4 Pool de campeões — P1

- **Objetivo:** recomendar um pool enxuto (2–3 campeões por rota) que maximize vitórias.
- **Dados:** histórico do usuário + meta do elo (7.5) + maestria.
- **Lógica:**
  - Score por campeão = WR suavizada do usuário (Seção 9) combinada com WR do campeão no meta do elo e tendência no patch.
  - Cobertura: o pool deve ter respostas para os campeões mais escolhidos na rota (matchups da Seção 7.6).
  - Diversidade de perfil (ex.: um seguro + um de carregar).
- **Saída:** "Pool sugerido" com justificativa por campeão + "campeões que estão te custando PDL".
- **Conformidade:** ✅ (são sugestões, o jogador decide)

### 7.5 Meta do elo (BR) — P1

- **Objetivo:** estatísticas de campeões **do elo do usuário no BR**, não globais.
- **Dados:** crawler (Seção 12).
- **Lógica:** por patch × tier agrupado (ex.: Ferro–Prata, Ouro–Platina, Esmeralda, Diamante, Mestre+) × rota:
  - Taxa de vitória, escolha e banimento.
  - Builds, runas e ordem de skills mais comuns e com maior WR.
  - WR por duração da partida (campeões de early vs. late).
- **Saída:** tier list do elo, página por campeão.
- **Conformidade:** ✅

### 7.6 Matchups e sinergias — P1

- **Objetivo:** como cada campeão se sai contra/com outros.
- **Dados:** crawler.
- **Lógica:**
  - **Matchup de rota**: WR de A vs. B na mesma rota; também diferença média de ouro @15 (mais estável que WR com amostra pequena).
  - **Sinergia de dupla** (ex.: ADC + suporte): WR quando juntos.
  - Exigir amostra mínima (Seção 9) e mostrar o `n`.
- **Saída:** "Contra X, quem vai bem" / "Com Y, quem combina".
- **Conformidade:** ✅

### 7.7 Assistente de seleção de campeões — P1

- **Objetivo:** ajudar na escolha **sem** violar o anonimato.
- **Entrada:** leitura automática via LCU (`/lol-champ-select/v1/session`, Seção 6.5): rota atribuída, campeões aliados/inimigos revelados e bans. Sem nomes de jogadores.
- **Ação opcional:** ao travar o campeão, importar runas, feitiços e conjunto de itens da opção escolhida (configurável: automático ou com um clique).
- **Lógica:**
  - Filtrar o pool do usuário pela rota.
  - Pontuar cada opção: matchup contra o oponente de rota (se revelado) + encaixe na composição (dano físico/mágico, frontline, engage, CC) + desempenho pessoal.
  - Sugestão de **bans** com base nos campeões que mais tiram PDL do usuário e na taxa de ban do elo.
- **Saída:** **2–3 opções** com motivo e números. Runas e build inicial por opção.
- **Conformidade:**
  - ✅ Usa apenas campeões (informação visível).
  - ❌ Nunca exibir nomes, elo ou histórico dos aliados.
  - ❌ Nunca sugerir dodge.
  - ❌ Nunca "auto-pick".

### 7.8 Builds, runas e ordem de skills — P1

- **Objetivo:** variações de build por matchup e composição inimiga.
- **Dados:** crawler (timeline → eventos `ITEM_PURCHASED` e `SKILL_LEVEL_UP`; detalhes → `perks`).
- **Lógica:**
  - Itens finalizados em ordem (ignorar componentes, consumíveis e `ITEM_UNDO`).
  - Agrupar por "caminho de build" (1º, 2º, 3º item) e calcular WR.
  - Condicionar à composição inimiga (ex.: ≥ 3 campeões de dano mágico).
- **Saída:** 2–3 caminhos de build com WR e `n`; página de runas; ordem de skills.
- **Conformidade:** ✅ (múltiplas opções = aumenta a diversidade de decisões)

### 7.9 Tela de carregamento (scouting pós-início) — P2

- **Objetivo:** contexto dos 10 jogadores quando a partida já começou.
- **Dados:** spectator-v5 + league-v4 + champion-mastery-v4 + nosso banco.
- **Lógica:** detectar partida ativa do usuário; para cada jogador: elo, WR na temporada, maestria e jogos recentes com o campeão escolhido; dicas do matchup de rota.
- **Saída:** painel de 10 cards + "dicas para sua rota".
- **Custo:** ~20–30 requisições por partida (balde `br1`).
- **Conformidade:** ✅ após o início (padrão de mercado). ❌ nada disso durante a seleção de campeões.

### 7.10 Painel de partida ao vivo — P1

- **Objetivo:** insights durante a partida usando **somente** a Live Client Data API (Seção 6) + dados que já temos no banco (meta e histórico).
- **Fonte:** `/activeplayer`, `/playerlist` e `/eventdata` a cada ~2s (é local, sem custo nem rate limit). Deduplicar eventos por `EventID`.
- **Saída:** janela do app (segunda tela ou alt-tab). Overlay por cima do jogo só em modo *janela sem bordas*; fica para depois.

#### 7.10.1 O que a API entrega (e o que não entrega)

| Disponível | Não disponível |
|---|---|
| **Você:** ouro atual, nível, atributos completos (AD, AP, armadura, RM, penetração, velocidade de ataque, vida…), nível de cada habilidade, runas completas | **Posição/coordenadas** de qualquer jogador (`position` é só a rota, ex.: `"MIDDLE"`) |
| **Os 10 jogadores:** campeão, nível, itens (com preço), placar (K/D/A, CS, `wardScore`), runa principal, feitiços (só o nome), time, se está morto | **Ouro dos outros** jogadores (só o seu) |
| **Eventos:** início, primeiro abate, abates de campeão (matador + assistências), multi-kills, ace, torres, inibidores, dragões (com o tipo e se foi roubado), arauto, barão, fim de jogo | **Recargas** (cooldowns) de habilidades e feitiços |
| **Jogo:** tempo de jogo, modo, mapa | Wards, visão, localização do jungler, dados da match-v5 (só chegam depois da partida) |

> Novos tipos de evento aparecem com novos objetivos (ex.: vastilarvas, Atakhan) → tratar eventos desconhecidos sem quebrar.

#### 7.10.2 Insights que vamos construir

| # | Insight | Como calcular | Prioridade |
|---|---|---|---|
| A | **Diferença de ouro "realizada"** por rota e por time | Soma de `price × count` dos itens de cada jogador (o ouro gasto é visível pelos itens). Para você, somar também o `currentGold`. Mostrar sua rota vs. oponente e time vs. time. | P1 |
| B | **Seu ritmo vs. referência** | CS, KDA, participação em abates (`(K+A) / abates do time`) e `wardScore` no minuto atual comparados à curva média **do seu elo com o seu campeão** (tirada dos frames das timelines do crawler) e à **sua** média. Ex.: "CS 58 aos 10:00 — referência do elo: 66". | P1 |
| C | **Opções de próximo item** | Perfil do time inimigo a partir dos itens visíveis + tags dos campeões: proporção de dano físico/mágico, quantos estão empilhando armadura/RM, quantos têm cura/vampirismo. Cruzar com os caminhos de build do meta (7.8) → **2–3 opções** com motivo e "faltam X de ouro". | P1 |
| D | **Ordem de habilidades** | Quando seu nível sobe, mostrar a ordem mais comum/com maior WR para o seu campeão e matchup (7.8). Só exibir — **nunca** upar automaticamente. | P1 |
| E | **Placar de objetivos** | Contagem a partir dos eventos: dragões por time e tipo (ponto de alma), arautos, barões, torres e inibidores destruídos. Só **contagem** — sem timer de reaparecimento. | P2 |
| F | **Resumo das suas mortes** | Eventos `ChampionKill` em que você é a vítima: quem matou, assistências, minuto. Ex.: "3 de 4 mortes envolveram o jungler inimigo". | P1 |
| G | **Metas pessoais ao vivo** | As metas do relatório semanal (7.3) viram barras de progresso na partida. Ex.: "Meta: ≥ 65 CS aos 10 min" / "Meta: ≤ 2 mortes antes dos 15". | P1 |
| H | **Lembrete do matchup no início** | No `GameStart`: seu histórico contra o campeão da sua rota e as dicas do matchup (7.6). Informação fixa do começo, não alerta durante o jogo. | P1 |
| I | **Fim de jogo instantâneo** | No `GameEnd`: salvar o último estado da Live Client para ter um resumo imediato enquanto a match-v5 não publica a partida. | P2 |

#### 7.10.3 Fora do painel (mesmo que daria para calcular)

- ❌ Timers de ultimate, de feitiços e de campos da selva.
- ❌ Alertas de power spike de inimigos (ex.: "Darius chegou ao nível 6").
- ❌ Alertas com o `respawnTimer` dos inimigos ("mid inimigo morto por 25s, vá agora") → transforma informação do placar em alerta de decisão. Não usar.
- ❌ Timers de reaparecimento de dragão/barão/arauto (zona cinza → fora).
- ❌ Comprar itens ou upar habilidades automaticamente.
- ❌ Ler memória do jogo (desde 6/10/2026 o Vanguard bloqueia isso para apps desconhecidos; nosso app usa só a API oficial local).
- ❌ Anúncios no painel/overlay.

- **Conformidade:** ✅ para os insights A–I, porque usam apenas dados expostos pela API oficial e apresentam informação/opções, não decisões. Revisar a cada atualização de política da Riot.

### 7.11 Desafios e maestria — P2

- **Objetivo:** benchmark e gamificação.
- **Dados:** lol-challenges-v1, champion-mastery-v4.
- **Saída:** "Seu percentil de visão é 34% — aqui está como subir"; metas de desafios.
- **Conformidade:** ✅

### 7.12 Preparação para Clash — P2

- **Objetivo:** estudar o time adversário do chaveamento.
- **Dados:** clash-v1 (time e jogadores) + match-v5 + champion-mastery-v4.
- **Saída:** campeões mais jogados de cada adversário, sugestões de bans e composição.
- **Conformidade:** ✅ (os times do Clash são públicos no chaveamento)

### 7.13 Duo e flex — P2

- **Objetivo:** com quem o usuário joga melhor.
- **Dados:** histórico (jogadores do mesmo time que se repetem = parceiros de grupo).
- **Lógica:** WR com cada parceiro recorrente; combinações de campeões da dupla.
- **Conformidade:** ✅ (dados do próprio grupo do usuário)

### 7.14 Gestão de sessão / tilt — P1

- **Objetivo:** evitar perder PDL em sequência.
- **Dados:** horários (`gameStartTimestamp`) e resultados do histórico.
- **Lógica:** WR por número do jogo na sessão (1º, 2º, 3º…), após X derrotas seguidas, por horário.
- **Saída:** "Depois de 2 derrotas seguidas sua WR cai para 38% — considere uma pausa." (Sugestão, não bloqueio.)
- **Conformidade:** ✅

---

## 8. Catálogo de métricas

### 8.1 Métricas da partida (match-v5 detalhes)

| Métrica | Fórmula / campo |
|---|---|
| KDA | `(kills + assists) / max(1, deaths)` |
| CS total | `totalMinionsKilled + neutralMinionsKilled` |
| CS/min | `CS total / (timePlayed / 60)` |
| Ouro/min | `challenges.goldPerMinute` ou `goldEarned / min` |
| Dano/min | `challenges.damagePerMinute` |
| % do dano do time | `challenges.teamDamagePercentage` |
| Participação em abates (KP) | `challenges.killParticipation` |
| Visão/min | `challenges.visionScorePerMinute` |
| Wards de controle | `detectorWardsPlaced` |
| Solo kills | `challenges.soloKills` |

### 8.2 Métricas da timeline

| Métrica | Como calcular |
|---|---|
| **CS@10 / CS@15** | `frames[10].participantFrames[id].minionsKilled + jungleMinionsKilled` (frame N ≈ minuto N) |
| **GD@10 / GD@15** (diferença de ouro na rota) | `totalGold` do usuário − `totalGold` do oponente de rota no frame |
| **XPD@10 / XPD@15** | Mesmo, com `xp` |
| **Curva de ouro do time** | Soma de `totalGold` dos 5 de cada time por frame |
| **Mortes precoces** | `CHAMPION_KILL` com `victimId` = usuário e `timestamp < 900000` (15 min) |
| **Morte sem visão** | Nenhum `WARD_PLACED` aliado a < ~1000 unidades da posição da morte nos 90s anteriores (heurística — wards destruídas antes não são consideradas com precisão) |
| **Morte isolada** | Nenhum aliado a < ~1500 unidades no frame mais próximo |
| **Participação em objetivos** | No `ELITE_MONSTER_KILL`, usuário a < ~2000 unidades (posição do frame mais próximo) |
| **Tempo até o 1º item lendário** | Primeiro `ITEM_PURCHASED` de um item finalizado (classificar via item.json: sem `into` e com custo alto) |
| **Ordem de skills** | Sequência de `SKILL_LEVEL_UP` (`skillSlot` 1–4 = Q/W/E/R) |
| **Roaming** | Posição fora da área da própria rota entre 3 e 14 min |

> As distâncias acima são **heurísticas iniciais**. Calibrar comparando com replays reais.

### 8.3 Regras de "pontos de melhoria" (exemplos para o pós-jogo)

```text
SE CS@10 < 0,8 × média_do_elo(campeão, rota)  → "Farm na fase de rotas abaixo do esperado"
SE mortes_antes_15 ≥ 3                         → "Muitas mortes cedo — revise trocas e posicionamento"
SE % mortes_sem_visão ≥ 50%                    → "Metade das mortes sem visão por perto"
SE wards_controle == 0 E duração > 20min       → "Nenhuma ward de controle comprada"
SE KP < 0,4 E rota != TOP                      → "Pouca participação nas lutas"
SE GD@15 > +1000 E derrota                     → "Ganhou a rota mas não converteu — revise objetivos/rotações"
```

Sempre mostrar o número real e a referência do elo junto da mensagem.

---

## 9. Estatística

Win rate com amostra pequena engana. Regras do projeto:

1. **Amostra mínima** para exibir um número: campeão no meta ≥ 200 jogos; matchup ≥ 100; build ≥ 50; dados pessoais ≥ 10 jogos (com aviso "amostra pequena" abaixo de 20).
2. **Sempre mostrar o `n`** ao lado da porcentagem.
3. **Suavização bayesiana** para ranquear (evita que 3/3 = 100% vire "melhor campeão"):
   `WR_suavizada = (vitórias + k × 0,5) / (jogos + k)`, com k ≈ 20 para dados pessoais e ≈ 100 para o meta.
4. **Intervalo de confiança** (Wilson) para comparar opções; se os intervalos se sobrepõem muito, dizer que "a diferença não é significativa".
5. **Separar por patch**: estatísticas de meta usam o patch atual (e o anterior se faltar amostra, com aviso).
6. **Excluir remakes**: `gameEndedInEarlySurrender == true` ou `gameDuration < 300s`.
7. Para dados pessoais, dar **mais peso a partidas recentes** (ex.: decaimento exponencial, meia-vida de 30 dias).

---

## 10. Arquitetura e stack

### 10.1 Visão geral — app desktop, tudo no PC do usuário (Fase 1)

```text
┌──────────────────────── App desktop (Windows) ────────────────────────┐
│                                                                        │
│  Interface (janela)  ◄──IPC──►  Processo principal (núcleo)            │
│  - perfil / histórico           ├─ módulo lcu/      ──► Cliente LoL    │
│  - seleção de campeões          │   (lockfile + WebSocket)  (LCU)      │
│  - painel em partida            ├─ módulo liveclient/ ──► Jogo :2999   │
│  - pós-jogo                     ├─ módulo riot/     ──► Riot API       │
│                                 │   (rate limit 2 baldes)  (internet)  │
│                                 ├─ fila de tarefas (import, crawler)   │
│                                 ├─ análise (métricas, agregações)      │
│                                 └─ SQLite local (partidas, meta)       │
└────────────────────────────────────────────────────────────────────────┘
          Data Dragon (CDN) ◄── sync a cada patch
```

**Fase futura (se for distribuir para outras pessoas):**

```text
App desktop ──► Nosso servidor (guarda a chave de produção) ──► Riot API
                  └─ crawler central + meta agregado (um só para todos)
```

> Por que o servidor só depois: a política proíbe colocar a chave em **binário distribuído**. Enquanto o app roda só no seu PC, a chave fica num arquivo de configuração local (fora do Git). Para distribuir, a chave tem que ficar no servidor, e o app passa a chamar o servidor.

### 10.2 Stack adotada

> **Decisão (17/11/2026):** a stack é **Tauri** (núcleo em Rust + interface web), escolhida por desempenho: o app roda ao lado do jogo e precisa ser leve em RAM e CPU. A coluna "Era no plano Electron" registra o equivalente original.

| Camada | Adotado (Tauri) | Era no plano Electron |
|---|---|---|
| Shell desktop | **Tauri 2** (Rust + WebView2 do Windows) | Electron + TypeScript |
| Núcleo ("processo principal") | Rust em `src/main/` (módulos `riot`, `lcu`, `liveclient`, `ddragon`, `db`, `analysis`, `jobs`, `ipc`) | Node em `src/main/` |
| Interface | React + Vite + TypeScript strict (`src/renderer/`) | React + Vite |
| IPC | Comandos e eventos Tauri tipados (`src/shared/ipc.ts` espelha os structs Rust) | `ipcMain`/`contextBridge` |
| Gráficos | ECharts (a partir da Fase 1) | ECharts ou Recharts |
| Mapa de mortes | Canvas/SVG sobre imagem do minimapa | — |
| LCU | módulo próprio em Rust: argumentos do processo / lockfile + WebSocket (`tokio-tungstenite`) | `league-connect` (Node) |
| Riot API | cliente próprio com rate limit em Rust (`reqwest` + limitador de janela deslizante, porte da Seção 3.4) | porte para TS |
| Banco | **SQLite** via `rusqlite` (SQLite embutido), JSON como TEXT | `better-sqlite3` |
| Análise | Rust puro (`src/main/analysis/`), funções sem I/O | TypeScript puro |
| Testes | Vitest (interface) + `cargo test` (núcleo), sempre com fixtures | Vitest |
| Empacotamento | bundler do Tauri (instalador NSIS .exe; auto-update pelo plugin `updater` no futuro) | electron-builder |
| Overlay (futuro) | janela transparente "sempre no topo" (só em modo janela sem bordas) | Overwolf (exige aprovação da Riot **e** da Overwolf) |

> O snippet da Seção 3.4 está em Python como referência de lógica; no app ele foi portado para Rust (`src/main/riot/`).

### 10.3 Princípios

- **Uma única instância do cliente Riot** controla o rate limit (os workers não podem chamar a API cada um por conta própria — senão estouram o limite).
- **Partidas brutas guardadas como JSON** (para reprocessar quando criarmos métricas novas, sem baixar de novo).
- **Agregações materializadas** (tabelas pré-calculadas por patch/elo/campeão) atualizadas por job, não calculadas a cada requisição.
- **Idempotência**: importar a mesma partida duas vezes não pode duplicar nada (chave primária = `matchId`).
- **O app não pode travar o jogo**: tarefas pesadas (crawler, agregações) rodam com prioridade baixa e **pausam durante a partida** (fase `InProgress`).
- **Tolerância a falhas locais**: cliente fechado, jogo fechado, LCU mudou de endpoint → o app degrada (mostra o que dá) em vez de quebrar.
- **Iniciar com o Windows (opcional)** e detectar o cliente sozinho, como Blitz/U.GG.

---

## 11. Modelo de dados

> Escrito em sintaxe PostgreSQL por clareza. No SQLite do app desktop: `JSONB` → `TEXT` (JSON), arrays (`INT[]`, `TEXT[]`) → `TEXT` com JSON, `TIMESTAMPTZ` → `INTEGER` (epoch ms), `BIGSERIAL` → `INTEGER PRIMARY KEY AUTOINCREMENT`.

```sql
-- Jogadores conhecidos
CREATE TABLE players (
  puuid            TEXT PRIMARY KEY,
  game_name        TEXT,
  tag_line         TEXT,
  platform         TEXT DEFAULT 'br1',
  profile_icon_id  INT,
  summoner_level   INT,
  last_updated     TIMESTAMPTZ
);

-- Histórico de elo (snapshots nossos — a API só dá o valor atual)
CREATE TABLE rank_snapshots (
  id          BIGSERIAL PRIMARY KEY,
  puuid       TEXT REFERENCES players(puuid),
  queue       TEXT,          -- RANKED_SOLO_5x5 / RANKED_FLEX_SR
  tier        TEXT,
  division    TEXT,
  lp          INT,
  wins        INT,
  losses      INT,
  taken_at    TIMESTAMPTZ DEFAULT now()
);

-- Partidas brutas (imutáveis)
CREATE TABLE matches (
  match_id       TEXT PRIMARY KEY,     -- ex.: BR1_3012345678
  queue_id       INT,
  game_version   TEXT,
  patch          TEXT,                 -- derivado: primeiros 2 números do game_version
  game_start     TIMESTAMPTZ,
  duration_s     INT,
  is_remake      BOOLEAN,
  avg_tier       TEXT,                 -- calculado quando possível
  raw            JSONB,                -- resposta completa de /matches/{id}
  timeline       JSONB                 -- resposta de /timeline (pode ser NULL)
);

-- Uma linha por participante (consulta rápida)
CREATE TABLE participants (
  match_id        TEXT REFERENCES matches(match_id),
  puuid           TEXT,
  participant_id  INT,                 -- 1..10 (liga com a timeline)
  team_id         INT,                 -- 100 / 200
  position        TEXT,                -- teamPosition
  champion_id     INT,
  win             BOOLEAN,
  kills INT, deaths INT, assists INT,
  cs INT, gold INT, damage INT, vision INT,
  items           INT[],
  runes           JSONB,
  summoners       INT[],
  PRIMARY KEY (match_id, participant_id)
);

-- Métricas derivadas da timeline (recalculáveis)
CREATE TABLE participant_metrics (
  match_id        TEXT,
  participant_id  INT,
  cs_at_10 INT, cs_at_15 INT,
  gd_at_10 INT, gd_at_15 INT,
  xpd_at_10 INT, xpd_at_15 INT,
  deaths_pre_15 INT,
  deaths_no_vision INT,
  objective_participation REAL,
  first_legendary_s INT,
  skill_order TEXT,                    -- ex.: "QEWQQRQEQEREEWWRWW"
  item_path INT[],                     -- itens finalizados em ordem
  metrics_version INT,                 -- para saber quando recalcular
  PRIMARY KEY (match_id, participant_id)
);

-- Agregados do meta (gerados por job)
CREATE TABLE meta_champion_stats (
  patch TEXT, tier_group TEXT, position TEXT, champion_id INT,
  games INT, wins INT, picks_rate REAL, ban_rate REAL,
  PRIMARY KEY (patch, tier_group, position, champion_id)
);

CREATE TABLE meta_matchups (
  patch TEXT, tier_group TEXT, position TEXT,
  champion_id INT, opponent_id INT,
  games INT, wins INT, avg_gd_15 REAL,
  PRIMARY KEY (patch, tier_group, position, champion_id, opponent_id)
);

-- Dados estáticos (Data Dragon)
CREATE TABLE static_champions (champion_id INT PRIMARY KEY, key TEXT, name TEXT, tags TEXT[], version TEXT);
CREATE TABLE static_items     (item_id INT PRIMARY KEY, name TEXT, gold INT, tags TEXT[], is_legendary BOOLEAN, version TEXT);
```

---

## 12. Crawler do meta do elo

### 12.1 Fluxo

```text
1. SEMENTE: league-v4 /entries/RANKED_SOLO_5x5/{tier}/{division}?page=N     (balde br1)
   → lista de PUUIDs por elo (amostrar páginas de forma aleatória/uniforme)
2. Para cada PUUID: match-v5 /by-puuid/{puuid}/ids?queue=420&startTime=<início do patch>&count=20
   (balde americas)
3. Deduplicar matchIds (o mesmo jogo aparece para até 10 jogadores)
4. Baixar /matches/{id} → salvar → (opcional) /timeline
5. Atribuir tier_group à partida (elo do jogador-semente ou média, quando disponível)
6. Job de agregação: recalcular meta_* por patch
```

### 12.2 Regras

- Rodar no **balde americas com prioridade baixa**; requisições do usuário passam na frente.
- Distribuir a amostra entre os grupos de elo de interesse (começar pelo elo do próprio usuário).
- Começar só com `/matches/{id}` (sem timeline) → ~3.000 partidas/hora com a chave de desenvolvimento. Baixar timeline só para as amostras que alimentam builds e ordem de skills.
- Parar e reiniciar a coleta a cada novo patch (filtrar pelo `startTime`).
- Com a chave de desenvolvimento expirando em 24h, o crawler precisa **tolerar 401/403** (pausar e avisar) — mais um motivo para pedir a Personal Key.

---

## 13. Roadmap por fases

### Fase 0 — Fundação (1–2 semanas)
- [ ] Projeto Tauri + React + TypeScript (janela abre, empacota em .exe)
- [ ] Configuração local da chave (arquivo fora do Git) + tela de "chave expirada"
- [ ] Cliente Riot com rate limit (2 baldes) + retry + logs
- [ ] Sync do Data Dragon (campeões, itens, runas) em pt_BR
- [ ] SQLite com as tabelas da Seção 11
- [ ] Módulo LCU: detectar o cliente, ler o usuário logado e a fase do jogo (`gameflow`)
- [ ] Importar as últimas 100 ranqueadas do usuário logado, com timeline

### Fase 1 — MVP pessoal (2–4 semanas)
- [ ] 7.1 Perfil e histórico
- [ ] 7.2 Análise pós-jogo — **abre sozinha no fim da partida** (`EndOfGame`)
- [ ] 7.14 Sessão/tilt
- [ ] Importação de runas/feitiços ao travar o campeão (com runas do seu próprio histórico, enquanto não há meta)
- [ ] Registrar o uso da LCU e **solicitar a Personal Key** no Developer Portal

### Fase 2 — Meta e decisão (4–6 semanas)
- [ ] Crawler local em segundo plano, pausando durante partidas (Seção 12)
- [ ] 7.5 Meta do elo · 7.6 Matchups · 7.8 Builds
- [ ] 7.4 Pool de campeões · 7.3 Relatório semanal
- [ ] 7.7 Assistente de seleção de campeões (automático via LCU)
- [ ] 7.10 Painel ao vivo: A, B, C, D, F, G, H

### Fase 3 — Ao vivo e extras
- [ ] 7.9 Tela de carregamento (spectator)
- [ ] 7.10 E e I (placar de objetivos, fim de jogo instantâneo)
- [ ] 7.11 Desafios · 7.12 Clash · 7.13 Duo
- [ ] Overlay (janela transparente) — avaliar

### Fase 4 — Distribuir (opcional)
- [ ] Servidor próprio com a chave + crawler central (Seção 10.1, fase futura)
- [ ] Registrar o produto, pedir **Production Key**, revisar a Seção 1.4
- [ ] Assinatura de código do instalador + auto-update

---

## 14. Pegadinhas conhecidas

- **Routing errado** é o erro nº 1: partidas e contas são `americas`; elo, maestria, spectator são `br1`.
- **`count` máximo de 100** nos IDs de partidas — paginar com `start`.
- **`startTime`/`endTime` em segundos**, mas os timestamps dentro da partida/timeline estão em **milissegundos**.
- **participantId (1–10)** liga a timeline aos participantes; times: 1–5 = 100 (azul), 6–10 = 200 (vermelho) — mas sempre confirmar pelo campo `participants` da própria timeline.
- **`teamPosition`** pode vir vazio em partidas estranhas (remakes, AFK) → descartar da análise de rota.
- **Remakes** distorcem tudo → filtrar (Seção 9, item 6).
- **Patch**: derivar de `gameVersion` (ex.: `15.22.712.1234` → `15.22`). A versão do Data Dragon tem formato parecido (`15.22.1`), mas a numeração comercial dos patches pode ser diferente — usar sempre os dois primeiros números do `gameVersion`.
- **Campos novos/removidos** em `challenges` e eventos (ex.: novos objetivos como vastilarvas e Atakhan) → código defensivo, tratar como opcionais.
- **Itens**: o Data Dragon traz itens de outros modos (ARAM, Arena) → filtrar por `maps["11"] == true` (Summoner's Rift).
- **Data Dragon atrasa** em dia de patch → manter a versão anterior até a nova aparecer.
- **Riot ID muda**: guardar sempre o PUUID e atualizar o nome periodicamente.
- **PUUID é por chave de API?** O PUUID é criptografado por projeto/chave em alguns contextos — ao trocar de chave (dev → personal/production), validar se os PUUIDs salvos continuam válidos e, se não, reconsultar por Riot ID.
- **Coordenadas do mapa**: ~0 a ~15000 nos dois eixos, origem no canto inferior esquerdo (base azul). Ao desenhar em imagem, inverter o eixo Y.
- **Partida demora a aparecer na match-v5** logo após o `EndOfGame` → tentar de novo com backoff (ex.: 30s, 1min, 2min, 5min).
- **Lockfile muda a cada abertura do cliente** (porta e senha novas) → nunca guardar; reler sempre.
- **Pasta do LoL não é sempre `C:\Riot Games`** → preferir ler os argumentos do processo `LeagueClientUx.exe`; deixar o usuário configurar a pasta como fallback.
- **Certificados autoassinados** (LCU e porta 2999) → aceitar **só** para `127.0.0.1`, nunca desligar a verificação de TLS globalmente.
- **Limite de páginas de runas**: se o usuário não tiver página livre, substituir apenas uma página criada pelo próprio app (prefixo no nome, ex.: `[App] Vayne ADC`), nunca as dele.

---

## 15. Referências

- Portal e lista de APIs: https://developer.riotgames.com/apis
- Documentação de LoL (routing, Data Dragon, Live Client Data, LCU): https://developer.riotgames.com/docs/lol
- Políticas gerais: https://developer.riotgames.com/policies/general
- Riot ID x nome de invocador: https://www.riotgames.com/en/DevRel/summoner-names-to-riot-id
- Postura da Riot sobre apps de terceiros: https://support-leagueoflegends.riotgames.com/hc/en-us/articles/225266848
- Regras de conformidade (Overwolf / LoL): https://dev.overwolf.com/ow-electron/guides/game-compliance/league-of-legends/
- Data Dragon (versões): https://ddragon.leagueoflegends.com/api/versions.json

> **Manutenção deste guia:** a Riot muda políticas e endpoints com frequência. Revisar a Seção 1 e a Seção 4 a cada temporada e sempre que um endpoint retornar erro inesperado.
