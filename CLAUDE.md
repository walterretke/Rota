# CLAUDE.md

App desktop (Windows) de análise para League of Legends ranqueado, no estilo Blitz / U.GG, focado em **análise pós-jogo, meta do elo e escolha de campeão**. Região BR1, fila principal Solo/Duo (queueId 420).

**O guia completo está em `docs/GUIA_DESENVOLVIMENTO.md`.** Leia a seção relevante antes de implementar qualquer módulo. Em caso de conflito, as regras deste arquivo valem.

## Regras inegociáveis

### 1. Conformidade com a Riot (guia, Seção 1)
Antes de implementar uma feature, verifique a Seção 1 do guia. **Nunca** implemente, nem como teste ou protótipo:
- Timers de ultimate, feitiços ou campos da selva; alertas de nível 6/power spike; rastreamento de jungler.
- Calculadora ou estimativa de MMR/ELO.
- Exibir nome, elo ou histórico dos **aliados** durante a seleção de campeões da Solo/Duo. Na seleção, use só `championId`, `assignedPosition` e `cellId`.
- Sugestão de dodge, auto-aceitar fila, auto-pick, auto-ban ou mensagens automáticas.
- Leitura de memória do jogo, injeção ou mods no cliente. Dados ao vivo vêm **só** da Live Client Data API (`127.0.0.1:2999`).
- Compra de itens ou upar habilidades automaticamente; alertas com o `respawnTimer` de inimigos; timers de reaparecimento de objetivos (lista completa no guia, Seção 7.10.3).

Toda recomendação mostrada ao usuário segue o formato **opções (2–3) + motivo + número com `n`**. Nunca uma ordem única ("pegue X").
Se uma tarefa pedida cair numa dessas regras, pare e avise em vez de implementar.

### 2. Chave da API
- Nunca hardcode a chave, nunca a coloque em código, teste, fixture, log ou mensagem de erro.
- Em dev: `.env` (`RIOT_API_KEY`). No app: config na pasta de dados do usuário (`%APPDATA%\br.rota.app\config.json`), nunca empacotada no instalador.
- A chave de desenvolvimento expira a cada 24h. Um `401`/`403` significa "regenerar a chave": pause a fila e avise, sem retry em loop.

### 3. Rate limit
- **Toda** chamada à Riot API passa pelo `RiotClient` único (`src/main/riot/`). Nunca chame `*.api.riotgames.com` direto, nem do Rust fora desse módulo nem do renderer.
- Os limites (20/1s e 100/2min) valem **por routing value**: `americas` (account, match) e `br1` (summoner, league, mastery, spectator…) são baldes separados.
- Partidas finalizadas são imutáveis: se já estão no banco, não baixe de novo.

### 4. Custo zero
- Nenhuma dependência de IA/LLM paga nem de serviço pago. Análises são estatística e regras determinísticas (guia, Seções 8 e 9).
- Não adicione SDKs de IA sem pedido explícito.

### 5. Testes não chamam a API real
- Testes usam fixtures em `tests/fixtures/` (JSON de partida/timeline com PUUIDs e nomes anonimizados).
- Nunca rode o crawler nem importações grandes contra a API real sem eu pedir.

## Stack
- **Tauri 2** (núcleo em Rust) + React + Vite + TypeScript strict
- SQLite via `rusqlite` (SQLite embutido, no lado Rust)
- LCU via módulo próprio em `src/main/lcu/` (argumentos do processo / lockfile + WebSocket)
- Gráficos: ECharts (a partir da Fase 1)
- Testes: Vitest (renderer e `src/shared`) + `cargo test` (núcleo)
- Empacotamento: bundler do Tauri (instalador NSIS `.exe`)

> Adaptação do plano original em Electron: `better-sqlite3` → `rusqlite`; `league-connect` → módulo próprio em Rust; electron-builder → bundler do Tauri; o "processo principal (Node)" é o núcleo Rust, que fica em `src/main/` (o crate Cargo e o `tauri.conf.json` moram lá).

## Estrutura
```
src/
  main/                 # núcleo Rust do Tauri (Cargo.toml, tauri.conf.json, lib.rs)
    riot/               # RiotClient, rate limiter, endpoints
    lcu/                # conexão com o cliente (processo/lockfile, WebSocket, gameflow)
    liveclient/         # Live Client Data API (127.0.0.1:2999)
    ddragon/            # sync de dados estáticos por patch
    db/                 # schema, migrations, repositórios
    analysis/           # métricas (puras, sem I/O), regras, agregações
    jobs/               # fila: import do usuário, crawler, agregação
    ipc/                # comandos e eventos expostos ao renderer
  renderer/             # UI React (nunca acessa rede ou banco direto)
  shared/               # tipos e contratos IPC compartilhados
tests/
  fixtures/
docs/
  GUIA_DESENVOLVIMENTO.md
```

## Comandos
```bash
npm run dev         # app em modo desenvolvimento
npm run build       # gera o instalador (.exe)
npm run test        # Vitest + cargo test
npm run lint        # ESLint + cargo clippy (-D warnings)
npm run typecheck   # tsc + cargo check
```
Antes de dizer que uma tarefa terminou, rode `typecheck`, `lint` e `test`.

## Convenções
- **Idioma:** textos da interface e mensagens ao usuário em **pt-BR**. Código (variáveis, funções, tipos) em **inglês**. Comentários podem ser em português.
- **Funções de análise são puras** (`src/main/analysis/`): recebem JSON da partida/timeline e devolvem métricas, sem I/O. Cada métrica nova vem com teste usando fixture.
- **O renderer nunca fala com a Riot, a LCU ou o banco.** Tudo passa por IPC tipado: comandos Tauri em `src/main/ipc/`, tipos em `src/shared/ipc.ts` (mantenha os dois em sincronia; structs Rust usam `serde(rename_all = "camelCase")`).
- **Identificadores:** use sempre o **PUUID** como chave. Exiba **Riot ID** (`gameName#tagLine`), nunca o antigo nome de invocador. O PUUID da LCU não é o da API: resolva a conta pelo Riot ID.
- **Routing:** `americas` para account-v1 e match-v5; `br1` para o resto. É o erro mais comum, então confira.
- **Unidades de tempo:** `startTime`/`endTime` da match-v5 em segundos; timestamps internos da partida/timeline em milissegundos.
- **Remakes:** exclua de toda estatística (`gameEndedInEarlySurrender` ou duração < 300s).
- **Amostra:** nunca mostre win rate sem o `n`; respeite as amostras mínimas da Seção 9.
- **Campos opcionais:** `challenges.*` e tipos de evento da timeline mudam entre patches, então trate como opcionais.
- **LCU:** é instável e não oficial. Encapsule tudo em `src/main/lcu/` e trate falhas sem derrubar o app. Escrita no cliente só em: páginas de runas (apenas as criadas pelo app, com prefixo `[App]`), feitiços (`my-selection`) e conjuntos de itens.
- **Desempenho:** jobs pesados (crawler, agregação, importação) pausam quando o `gameflow` está em `InProgress` (`jobs::wait_while_in_game`).
- **TLS:** aceite certificado autoassinado só para `127.0.0.1` (LCU e porta 2999). Nunca desligue a verificação globalmente.
- **Dependências:** não adicione pacotes (npm ou crates) novos sem justificar; prefira o que já está na stack.

## Fase atual
Fase 0 — Fundação (guia, Seção 13). Atualize esta linha ao mudar de fase.
