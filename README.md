# Rota

App desktop para Windows que ajuda a evoluir na ranqueada de League of Legends, no estilo Blitz / U.GG: análise pós-jogo, meta do seu elo e escolha de campeão. Servidor principal: Brasil (BR1), fila Solo/Duo.

Feito com **Tauri** (núcleo em Rust + interface React), para ficar leve e não pesar no jogo.

> **Status:** Fase 0 (fundação). O app já detecta o cliente do LoL e a fase do jogo, guarda sua chave da Riot com segurança, baixa os dados do patch (Data Dragon) e importa suas últimas 100 ranqueadas com a linha do tempo. A análise pós-jogo vem na Fase 1.

---

## Como rodar no Windows

### 1. Instale os pré-requisitos (uma vez só)

1. **Microsoft C++ Build Tools**: baixe em <https://visualstudio.microsoft.com/visual-cpp-build-tools/>, abra o instalador e marque **"Desenvolvimento para desktop com C++"** (Desktop development with C++). Pode levar uns minutos.
2. **Rust**: baixe e rode o `rustup-init.exe` de <https://rustup.rs/>. Aceite as opções padrão (toolchain `stable-x86_64-pc-windows-msvc`).
3. **Node.js 22 LTS** (ou mais novo): <https://nodejs.org/>.
4. **Git**: <https://git-scm.com/download/win>.
5. **WebView2**: já vem no Windows 10 e 11 atualizados. Se o app abrir em branco, instale o "Evergreen Runtime" em <https://developer.microsoft.com/microsoft-edge/webview2/>.

Feche e abra o terminal depois de instalar, para ele enxergar os programas novos. Confira com:

```powershell
rustc --version
node --version
```

### 2. Baixe o projeto e as dependências

```powershell
git clone https://github.com/walterretke/Rota.git
cd Rota
npm install
```

### 3. Abra o app em modo desenvolvimento

```powershell
npm run dev
```

A primeira vez demora (3 a 6 minutos), porque o Rust compila tudo. Depois fica rápido. A janela do Rota abre sozinha.

### 4. Configure a chave da Riot

1. Entre em <https://developer.riotgames.com/>, faça login e copie a **Development API Key** (`RGAPI-...`).
2. No Rota, vá em **Configurações**, cole a chave em "Nova chave" e clique em **Salvar e testar chave**.
3. Se quiser importar partidas com o cliente do LoL fechado, preencha também o **Riot ID** (ex.: `retke#retke`).

A chave fica só no seu computador, em `%APPDATA%\br.rota.app\config.json`. Ela nunca vai para o Git, para o instalador ou para os logs. A chave de desenvolvimento **expira a cada 24 horas**: quando isso acontecer, o Rota mostra um aviso vermelho e para de chamar a Riot. É só gerar outra no portal e colar de novo.

> Para desenvolvimento também dá para usar um arquivo `.env` na raiz (copie de `.env.example` e preencha `RIOT_API_KEY`). Ele só é lido no modo `npm run dev` e está no `.gitignore`.

### 5. Teste jogando

1. Abra o cliente do LoL. Em até 5 segundos o card **Cliente do LoL** mostra "Conectado" e sua conta.
2. Entre numa fila: a fase muda sozinha (Lobby, Procurando partida, Seleção de campeões, Em partida, Fim de jogo).
3. Clique em **Importar últimas 100 ranqueadas**. A primeira importação leva uns 5 minutos com a chave de desenvolvimento (limite da Riot: 100 chamadas a cada 2 minutos). Partidas já baixadas nunca são pedidas de novo.
4. Se você entrar numa partida durante a importação, ela **pausa sozinha** e continua quando o jogo acabar.

### 6. Gerar o instalador (.exe)

```powershell
npm run build
```

O instalador sai em `src\main\target\release\bundle\nsis\Rota_0.1.0_x64-setup.exe`. Ele instala só para o seu usuário, sem pedir administrador.

---

## Onde ficam os arquivos

| O quê | Onde |
|---|---|
| Configurações (inclui a chave) | `%APPDATA%\br.rota.app\config.json` |
| Banco de dados (partidas, elo, dados do patch) | `%APPDATA%\br.rota.app\rota.db` |
| Logs | `%LOCALAPPDATA%\br.rota.app\logs\` |

## Problemas comuns

- **"Cliente do LoL: Fechado" com o cliente aberto:** se o LoL foi aberto como administrador, o Rota (rodando como usuário comum) não consegue ler o processo. Abra os dois do mesmo jeito, ou informe a pasta do LoL em Configurações (ex.: `C:\Riot Games\League of Legends`) para o Rota usar o arquivo `lockfile`.
- **Aviso de chave expirada:** gere uma nova chave no portal da Riot e cole em Configurações.
- **"Conta não encontrada":** confira o Riot ID no formato `Nome#TAG`.
- **Erro de compilação `link.exe not found`:** faltou o passo 1.1 (C++ Build Tools).

## Para quem desenvolve

```powershell
npm run test        # Vitest + cargo test (só fixtures, nunca a API real)
npm run lint        # ESLint + cargo clippy
npm run typecheck   # tsc + cargo check
```

As regras do projeto (conformidade com a Riot, chave, rate limit, convenções) estão no [CLAUDE.md](CLAUDE.md), e o guia completo em [docs/GUIA_DESENVOLVIMENTO.md](docs/GUIA_DESENVOLVIMENTO.md).

O Rota não é endossado pela Riot Games e não reflete as opiniões da Riot Games nem de ninguém oficialmente envolvido na produção ou gestão de League of Legends. League of Legends e Riot Games são marcas registradas da Riot Games, Inc.
