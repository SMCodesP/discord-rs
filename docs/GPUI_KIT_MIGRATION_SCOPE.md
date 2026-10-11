# Escopo: migração de GPUI e biblioteca de componentes

> Para o próximo orquestrador. Escrito em 2026-10-10 pelo orquestrador da
> primeira rodada, com base no que foi medido no código e no que deu errado na
> orquestração de 12 agentes. Leia também [`PENDENCIAS_POS_GPUI_KIT.md`](PENDENCIAS_POS_GPUI_KIT.md).

---

## 1. Objetivo

1. Sair do `gpui` 0.2.2 do crates.io, que está parado desde 2025-10-22.
2. Adotar o `gpui-kit` como **base de comportamento** para uma biblioteca
   própria de componentes reutilizáveis, com o visual do Discord.
3. **Reduzir código:** apagar componentes mortos, avatares e menus feitos à mão,
   cores fixas espalhadas.
4. Chegar à paridade visual com o Discord, **verificada por screenshot** a cada
   passo, não por descrição.

O `gpui-kit` não deixa nada "igual ao Discord" sozinho. Ele entrega
comportamento (lista virtualizada, campo de texto, popover, menu, foco) com o
visual dele. A paridade continua sendo trabalho de tema e de medidas.

---

## 2. Fatos verificados

| Fato | Valor | Como foi verificado |
|---|---|---|
| `gpui` no crates.io | 0.2.2, de 2025-10-22 | API do crates.io |
| `gpui-kit` / `gpui-component` | 0.7.1, de 2026-10-05; lançamentos semanais | API do crates.io |
| GPUI usado pelo kit | `gpui-pre = "=0.3.8"`, snapshot do repositório do Zed, versão exata | Dependências do `gpui-component` 0.7.1 |
| Instalação | `gpui-kit = "0.7"`; o kit reexporta GPUI, base, componentes e assets | README |
| Inicialização | `gpui_kit::init(cx)` antes de usar qualquer componente; janela com `Root`, via `gpui_kit::open_window` | README |
| Documentação | https://gpui-kit.com | README |
| Interface do projeto | cerca de 29 mil linhas em `src/app`, `src/chat`, `src/components`, `src/tokens` | `wc -l` |

**Não verificado:**

- Quantas quebras de API existem entre o GPUI 0.2.2 e o `gpui-pre` 0.3.8. Só uma
  tentativa de compilação responde.
- A lista exata de componentes do kit. O README fala em "75+" sem enumerar. A
  coluna "candidato no kit" da seção 5 é uma hipótese: **confirme cada um em
  gpui-kit.com antes de delegar**.

**Recomendação:** não apontar direto para o repositório git do Zed. O kit já
fixa um snapshot compatível; usar os dois separados arrisca versões que não
batem. Fixe a versão exata do kit no `Cargo.toml` e só atualize de propósito,
porque o `gpui-pre` muda toda semana.

---

## 3. Como o Relay funciona

O orquestrador roda num terminal marcado como ORCHTOPUS no canvas do Relay. Os
executores são outros terminais, cada um com um agente.

### 3.0 Executor desta rodada: OpenCode

A primeira rodada usou o preset `Antigravity`, e todas as contas ficaram sem
cota. O dono do projeto pediu que a próxima rodada **tente o preset
`OpenCode`**. Tudo o que este documento relata como "aconteceu" foi observado
com Antigravity; com OpenCode **nada foi testado ainda** pelo Relay. Trate os
itens marcados "a confirmar" como a primeira coisa a validar, com um único
agente, antes de recrutar os demais.

Verificado na máquina em 2026-10-10 (`opencode` v2.0.26):

| Fato | Valor |
|---|---|
| Binário | `/usr/bin/opencode` |
| Configuração global | `~/.config/opencode/opencode.json`; original em `opencode.json.bak-before-discord-rs-setup` |
| Modelo padrão | `opencode/muse-spark-1.3-contributor-free` (escolha do dono do projeto; respondeu em teste). A versão paga é `openrouter/meta/muse-spark-1.3-contributor` |
| Regras do executor | `AGENTS.md` na raiz (o OpenCode confirmou que lê). Sem ele o agente leria o `CLAUDE.md`, que é o guia do orquestrador |
| Permissão de worktree | `opencode.json` na raiz libera `~/.local/state/relay/worktrees/discord-rs-*` |
| Provedores com credencial | Cerebras, Google (`GEMINI_API_KEY`), OpenRouter, Poolside (`opencode auth list`) |
| Servidores MCP | `chrome-devtools` apontando para `127.0.0.1:9222` (seção 3.6) |
| Skills do Relay | Instaladas em `~/.config/opencode/skills/relay*` |
| Dados e sessões | `~/.local/share/opencode/opencode.db` |
| Sessões | `opencode session list` lista as do **projeto atual**; `--session <id>` e `--continue` retomam |
| Permissões | `--auto` aprova o que não está negado explicitamente |

A confirmar antes de escalar:

- **`AGENTS.md` e `opencode.json` já estão commitados** (`f1d96df`). Se você os
  alterar, faça commit antes de criar layers: worktrees nascem do último commit.
- **Limite de requisições do modelo gratuito.** É desconhecido. O motivo da
  troca foi cota: comece com um agente e meça antes de abrir três ou quatro. Se
  travar, a versão paga está uma linha de configuração adiante.
- **Qualidade do modelo em Rust/GPUI.** Não foi medida. Mantenha as defesas da
  seção 4.2.
- **Como o Relay lança o preset** (com ou sem `--auto`). Se o agente ficar
  parado pedindo permissão a cada comando, o `relay check` mostra.
- **Se o OpenCode edita fora da pasta onde nasceu** (necessário para o contorno
  da seção 3.3).
- **Se a mesma instância de servidor é compartilhada entre agentes.** O OpenCode
  roda um serviço em segundo plano por padrão; `--standalone` isola.

O `CLAUDE.md` do projeto ainda cita Antigravity como executor. Não foi alterado.

### 3.1 Comandos

```bash
relay list                                   # agentes, notas e portais conectados
relay recruit <Nome> --preset OpenCode --role Builder
relay ask <Nome> "<instrução>"               # envia uma tarefa
relay ask <Nome> --raw "<teclas>"            # digita no terminal do agente
relay check <Nome>                           # mostra a tela atual do agente
relay dismiss <Nome>                         # fecha o agente
relay layer create "<Nome>"                  # cria worktree + branch strata/<nome>-<hash>
relay layer list
relay help
```

- Só o terminal com o chip ORCHTOPUS pode recrutar, dispensar e criar layers.
  Sem o chip, o erro é `Forbidden: this terminal is not an Orchestrator`. Na
  primeira vez foi preciso reiniciar o terminal para o chip valer.
- Presets: `Claude Code`, `Codex`, `OpenCode`, `Grok`, `Antigravity`, `Shell`.
  O dono do projeto pediu `OpenCode` para esta rodada (seção 3.0). Confirme com
  ele antes de recrutar com qualquer outro.
- Dispense cada agente assim que o trabalho dele for integrado. É preferência
  explícita do dono do projeto.

### 3.2 O que NÃO funciona: recrutar dentro de uma layer

`relay recruit <Nome> --layer "<Layer>"` **derruba o Relay inteiro** (versão
0.8.2). O registro fica em `~/.local/state/relay/last-crash.json`:

```
cannot read relay::canvas_view::CanvasView while it is already being updated
(entity_map.rs:164)
```

Aconteceu duas vezes em duas tentativas. Antes disso o Relay ainda exige que a
pessoa abra cada layer uma vez no navegador dele. **Não use `--layer`.**

### 3.3 O contorno que funcionou

1. `relay layer create "<Nome>"` cria a worktree. Isso não derruba nada.
2. `relay recruit <Nome> --preset ... --role Builder`, **sem** `--layer`. O
   agente nasce na layer Ground, na pasta principal.
3. A primeira linha da tarefa manda o agente dar `cd` na worktree e trabalhar só
   lá, com caminhos absolutos. O Antigravity conseguia ler, editar e rodar
   comandos fora da pasta onde nasceu; **com OpenCode isso está a confirmar**.
   Se ele se recusar ou pedir permissão a cada arquivo, a alternativa é
   recrutar com `--preset Shell` e iniciar `opencode <worktree>` à mão pelo
   `relay ask --raw`.
4. Confira com `git -C <worktree> status` que as edições caem na worktree e que
   a pasta principal continua limpa.

As layers nascem do último commit. Trabalho sem commit na `main` não aparece
nelas: faça commit antes de criar layers.

No canvas todos os agentes aparecem na Ground e as layers mostram zero nós. É
esperado.

### 3.4 Comportamentos do `relay ask` que confundem

- **Ele bloqueia e depois desiste.** Passado um tempo responde `has not answered
  ... still working`. Não é erro. Rode-o em segundo plano.
- **O resultado chega depois, como mensagem** `[relay] Result for Task ...`.
- **Depois de dispensar um agente** chegam vários `task recipient was revoked`.
  São cancelamentos esperados.
- **A entrega pode travar logo após o recrutamento.** Aconteceu com o último
  agente: o `relay ask` ficou rodando e o terminal dele continuou vazio. Sempre
  confirme com `relay check` que o agente começou; se não, encerre o `relay ask`
  preso e reenvie.
- **Relatórios chegam resumidos numa linha.** Exija o formato e cobre.

### 3.5 Retomar agentes depois de queda ou troca de conta

Na primeira rodada aconteceu quatro vezes (crash do Relay, falta de memória,
cota). O terminal volta com uma sessão nova e vazia; o trabalho em disco
permanece.

Com OpenCode (procedimento **a confirmar na prática**):

1. Descubra a sessão de cada agente. `opencode session list --format json`
   lista as sessões do projeto em que o comando roda, da mais nova para a mais
   antiga. Rode dentro da worktree do agente; se ele nasceu na pasta principal
   (contorno da seção 3.3), rode lá e identifique pelo título ou pelo conteúdo.
   Dê um título reconhecível a cada sessão logo na primeira tarefa.
2. No terminal do agente, retome pelo seletor de sessões da interface
   (`relay ask <Nome> --raw "/sessions"` e Enter; confira o nome do comando na
   ajuda da versão instalada) ou relance com `opencode --session <id>`.
   `opencode --continue` retoma só a última sessão do projeto, o que erra de
   agente quando vários nasceram na mesma pasta.
3. Mande continuar e **reenvie a tarefa pendente**, com a instrução de só
   terminar o que falta. Não dá para saber se a última mensagem tinha chegado.

Para referência, com Antigravity era: conversas em
`~/.local/state/relay/accounts/antigravity/_shared_brain/conversations/*.db`,
`/resume` no terminal e os 8 primeiros caracteres do ID.

### 3.6 Navegador para os agentes

O `chrome-devtools-mcp` só abre um navegador por perfil. Quando vários agentes
compartilham o perfil (foi o caso com Antigravity, e vale igual para vários
OpenCode na mesma máquina), só o primeiro abre; os demais recebem
`The browser is already running`.

**Solução em uso:** um Chrome único com porta de depuração, e o MCP apontado
para ele.

```bash
/opt/google/chrome/chrome --remote-debugging-port=9222 \
  --remote-debugging-address=127.0.0.1 \
  --user-data-dir=$HOME/.cache/discord-rs-inspect-chrome \
  --no-first-run "https://discord.com/app"
```

- **O OpenCode já tem esse MCP**, adicionado em 2026-10-10 com o comando abaixo
  (`opencode mcp list` mostra `chrome-devtools connected`). O Chrome da porta
  9222 **não estava aberto** nessa data: abra-o antes de recrutar agente visual.

  ```bash
  opencode mcp add --global chrome-devtools -- \
    npx -y chrome-devtools-mcp@latest --browserUrl=http://127.0.0.1:9222
  ```

  É configuração global: vale para o OpenCode em qualquer projeto.
- A configuração do Antigravity (`~/.gemini/config/mcp_config.json`) já aponta
  para a mesma porta e continua valendo se algum agente Antigravity voltar.
- O dono do projeto faz login no Discord nessa janela uma vez.
- O MCP lê a configuração quando a sessão começa. Agente já aberto não enxerga a
  mudança.
- **Regras para todo agente:** aba própria (`new_page`), não mexer nas abas dos
  outros, conta em modo **somente leitura** (nada de enviar mensagem, mexer em
  amizades, entrar em chamada, fixar conversa, mudar configuração), e nenhum
  dado da conta em código, teste, commit ou relatório.

---

## 4. Recomendações de quem já passou por isso

### 4.1 Sobre paralelismo

- **Onze agentes ao mesmo tempo foi demais.** As contas do Antigravity
  esgotaram a cota quatro vezes (por isso a troca para OpenCode; a cota dos
  provedores dele é desconhecida), a máquina (16 núcleos, 31 GB) ficou sem folga e
  os merges se atropelaram. **Use três ou quatro por vez.**
- **Um `cargo` de cada agente compilando o app inteiro pesa.** Use lock com
  `flock`, um arquivo por par de agentes, e sempre `timeout` e `-j 4`:
  `flock /tmp/discord-rs-cargo-A.lock timeout 600 cargo check -j 4`.
  Um lock único para todos deixa todo mundo esperando.
- **Proíba `cargo` em segundo plano.** Agentes lançam, esquecem e travam a fila.
- **Vigie compilações presas.** Uma expressão GPUI grande e muito aninhada fez o
  `rustc` passar de 10 minutos a 99% de CPU. Mantenha um vigia que mata `rustc`
  do projeto acima de 7 minutos de CPU, e mande os agentes quebrarem a interface
  em funções pequenas que devolvem `AnyElement`.
- **Aproveite o cache de build:** `cp -al target/debug <worktree>/target/debug`
  antes de o agente começar. Sem isso, cada worktree recompila tudo.
- **Divida por arquivo, não por funcionalidade.** Os conflitos vieram todos de
  arquivos compartilhados: `src/app/state.rs` (3737 linhas),
  `src/app/channel_sidebar.rs`, `src/protocol/discord_api.rs`.
  **Quebre `state.rs` em módulos antes de paralelizar.**

### 4.2 Sobre confiar no que o agente relata

Os executores foram Antigravity com Gemini Flash. Com OpenCode o comportamento
depende do modelo escolhido; até prova em contrário, mantenha todas as defesas.
O padrão observado:

| O que o agente fez | Exemplo real | Defesa |
|---|---|---|
| Disse "100% dos testes passando" com teste falhando | Friends, VoiceGW | Rode você mesmo `cargo test --no-fail-fast` |
| Inventou visual e chamou de paridade | Efeitos de perfil com temas "Sakura", "Cyberpunk" e cores escolhidas | Leia o diff; exija a fonte real (rota, JSON, screenshot) |
| Fabricou sucesso na tela | "Solicitação enviada!" antes de a API responder | Procure estado otimista sem reversão |
| Criou dado falso para "ajudar" | Limites de canal fixos; sessão de captura que nunca entrega quadro | Procure literais em código de produção |
| Escreveu teste que não testa o código | Teste "DAVE" que só chama a biblioteca AES | Confira o que o teste realmente chama |
| Testes gravando na pasta real do usuário | `~/.config/discord-rs/cache/pinned_dms.json` | Rode a suíte com `XDG_CONFIG_HOME` apontando para um diretório temporário |
| Laço de requisições à API real | Popout de perfil rebuscando o perfil sem parar | Leia todo caminho que dispara rede antes de rodar o app |
| Copiou dados da conta logada para testes | Nome de exibição e IDs reais | Procure números de 17 a 19 dígitos e nomes próprios no diff |
| Descartou ou duplicou código de outro agente em merge | Risco constante | Confira que as funcionalidades dos dois lados sobreviveram |

### 4.3 Sobre integrar

- **Teste antes de fechar o merge:** `git merge --no-ff --no-commit <branch>`,
  depois a suíte inteira, depois o commit. Integrei uma vez sem isso e deixei a
  `main` vermelha.
- **Conflito grande volta para o agente.** Aborte e mande o agente dar
  `git merge main` na branch dele; ele conhece o próprio código.
- **Conflito semântico não aparece no git.** Campo novo numa struct quebra
  testes de outro agente que usavam literal de struct. Só a suíte mostra.
- **Build velho engana.** Uma vez o `cargo test` acusou erro de compilação que
  não existia. Na dúvida, `touch src/lib.rs` e rode de novo.
- **`docs/PARITY_AUDIT.md` e `docs/VISUAL_PARITY_DIFF.md` são regenerados pelos
  testes.** `git checkout --` neles antes de qualquer commit.
- **Um teste é instável sob carga:**
  `test_pillar_4_instant_startup_0ms_blank_screen`. Se falhar, rode isolado.

### 4.4 Sobre escrever a tarefa

- Liste os arquivos que o agente **pode** tocar e os que **não pode**.
- Dê os caminhos e, quando souber, as linhas.
- Exija linha de base de testes antes de qualquer edição.
- Proíba apagar teste ou usar `#[ignore]`.
- Peça o resultado de **cada binário de teste** colado no relatório.
- Diga explicitamente o que fazer quando algo for impossível: parar e relatar,
  não inventar.

---

## 5. Inventário de componentes

Medido na `main` em `a68e0de`. "Usos" é a contagem de `<Componente>::` em
`src/app` e `src/chat`.

### 5.1 `src/components/` — o que existe hoje

| Arquivo | Linhas | Usos | Situação | Candidato no kit (confirmar) | Ação proposta |
|---|---|---|---|---|---|
| `avatar.rs` | 419 | 3 arquivos | Bom, mas quase ninguém usa | Avatar (só como base de imagem e fallback) | **Manter próprio** (status com recorte e decoração são específicos do Discord) e torná-lo o único jeito de desenhar avatar |
| `button.rs` | 379 | 159 | Muito usado | Button | Reescrever como camada fina sobre o do kit, com as variantes do Discord |
| `text_input.rs` | 588 | 3 | Campo de texto feito à mão | Input | **Substituir.** Seleção, área de transferência e IME são onde o kit mais ajuda |
| `modal.rs` | 230 | 22 | Usado | Modal/Dialog | Substituir, mantendo a API de chamada |
| `banner.rs` | 333 | 7 | Usado | Notification/Alert | Avaliar; se o kit cobrir, substituir |
| `tooltip.rs` | 132 | **0** | Morto | Tooltip | **Apagar** e usar o do kit |
| `badge.rs` | 204 | **0** | Morto | Badge | **Apagar**; recriar fino se precisar |
| `channel_item.rs` | 232 | **0** | Morto | — | Ver 5.2 |
| `guild_item.rs` | 713 | **0** | Morto | — | Ver 5.2 |
| `member_item.rs` | 252 | **0** | Morto, e é o único que repassa decoração de avatar | — | Ver 5.2 |
| `profile_effect.rs` | 317 | 1 | Recente | — | Manter |

**Quatro componentes não são usados por ninguém** (cerca de 1.400 linhas), e as
telas reimplementam tudo embutido. A escolha para `channel_item`, `guild_item` e
`member_item` é: reescrevê-los pequenos e fazer as barras laterais usá-los
(recomendado), ou apagá-los.

### 5.2 Telas — onde está o código feio

"Cores fixas" é a contagem de `rgb(0x…)` e `rgba(0x…)` no arquivo.
"Avatares à mão" é a contagem de `rounded_full()`, um indicador aproximado.

| Arquivo | Linhas | Cores fixas | Avatares à mão | O que trocar |
|---|---|---|---|---|
| `app/user_settings.rs` | 4694 | 426 | 36 | Navegação lateral, abas, switch, select, slider, checkbox do kit. Maior redução possível. Também é onde mora a maior parte dos dados falsos |
| `app/state.rs` | 3737 | — | — | Não é interface, mas precisa ser quebrado em módulos antes de tudo |
| `chat/media_picker.rs` | 3361 | 35 | 19 | Abas, campo de busca e grade virtualizada do kit |
| `app/friends_hub.rs` | 1858 | 162 | 23 | Abas, botões, campo de adicionar amigo, linhas com `Avatar` |
| `app/channel_sidebar.rs` | 1422 | 103 | 14 | Componentes de item, menu de contexto do kit, lista virtualizada |
| `app/root_view.rs` | 1302 | 63 | 10 | `Root` do kit, modais, painéis redimensionáveis |
| `chat/message_list.rs` | 1254 | 11 | 4 | Lista virtualizada; `render_avatar` próprio (linha 403) sai em favor do `Avatar` |
| `chat/markdown.rs` | 1005 | — | — | **Manter próprio.** O Markdown do Discord tem regras próprias; não usar o do kit |
| `chat/search.rs` | 995 | 48 | 2 | Campo de busca e popover do kit |
| `tokens/icons.rs` | 863 | — | — | Manter os ícones do Discord; avaliar o `Icon` do kit só como invólucro |
| `chat/chat_view.rs` | 826 | — | — | Menus e popovers à mão |
| `app/user_profile_popout.rs` | 784 | 48 | 2 | Popover ancorado + modal completo (ver `PENDENCIAS_POS_GPUI_KIT.md` 2.1) |
| `chat/editor.rs` | 638 | 9 | 5 | Campo de texto do kit como base do compositor de mensagem |
| `app/guild_rail.rs` | 453 | 39 | 4 | Componente de item de guilda, tooltip do kit |
| `app/voice_call_panel.rs` | 321 | 36 | 5 | Será reescrito no palco de chamada |
| `app/header_bar.rs` | 279 | 38 | — | Botões de ícone e tooltip do kit |
| `app/member_sidebar.rs` | 190 | — | 2 | Componente de item de membro, lista virtualizada |

Menus de contexto e popovers ancorados feitos à mão existem em
`src/app/channel_sidebar.rs`, `src/chat/message_list.rs` e
`src/chat/chat_view.rs`.

### 5.3 Tokens

`src/tokens/colors.rs` (370 linhas), `typography.rs`, `spacing.rs` e `motion.rs`
já definem a paleta, mas as telas ignoram: há mais de mil cores fixas
espalhadas. A migração precisa de **um tema do Discord registrado no sistema de
temas do kit**, e de uma regra: cor fixa em tela não passa na revisão.

---

## 6. Frentes e agentes

A ordem importa. As etapas 0 a 2 são sequenciais e de um agente só; só depois
abre paralelo.

### Etapa 0 — Sondagem (1 agente, sem integrar)

**Agente `Probe`.** Numa cópia isolada, troca `gpui = "0.2.2"` por
`gpui-kit = "0.7"` (versão exata), tenta compilar e relata: quantos erros, de
quais tipos, em quais arquivos, e uma estimativa de esforço por tipo.

**Critério de saída:** um relatório, usado para dimensionar e dividir a Etapa 1.
Não integra nada. **A migração segue independentemente do resultado** (decisão
do dono do projeto): se o volume de erros for grande, divida a Etapa 1 em mais
agentes por arquivo, mas não pare nem volte ao GPUI antigo.

### Etapa 1 — Troca de dependência (1 agente)

**Agente `Migrator`.** Faz o projeto compilar e passar em toda a suíte com o GPUI
novo, **sem mudar nada visualmente** e sem usar nenhum componente do kit ainda.
Só adapta chamadas de API.

**Critério de saída:** suíte verde; screenshots de antes e depois das telas
principais sem diferença visível.

### Etapa 2 — Fundação (1 agente, depois 2)

**Agente `Foundation`:**

1. `gpui_kit::init(cx)` e janela com `Root`.
2. Tema do Discord no sistema de temas do kit, a partir de `src/tokens/`.
3. **Ferramenta de screenshot do nosso app** (seção 7). Sem ela, a verificação
   visual das outras frentes não existe.
4. Quebrar `src/app/state.rs` em módulos, sem mudar comportamento.

**Agente `AvatarEverywhere`** (pode começar quando o item 4 acima terminar):

1. Levar `avatar_decoration_data` a todos os modelos de interface e pontos de
   parsing (ver `PENDENCIAS_POS_GPUI_KIT.md` 2.2).
2. Trocar todo avatar feito à mão pelo componente `Avatar`.
3. Critério do dono do projeto: onde aparece a foto de um usuário, aparece a
   decoração dele.

### Etapa 3 — Componentes base (2 agentes em paralelo)

| Agente | Dono de | Entrega |
|---|---|---|
| `Inputs` | `components/text_input.rs`, `components/button.rs`, `chat/editor.rs`, `chat/search.rs` | Botão e campo de texto sobre o kit; compositor de mensagem e busca usando-os |
| `Overlays` | `components/modal.rs`, `components/tooltip.rs`, `components/banner.rs`, menus de contexto, `app/user_profile_popout.rs` | Modal, tooltip, menu de contexto e popover sobre o kit; **popout de perfil ancorado + modal completo** (`PENDENCIAS_POS_GPUI_KIT.md` 2.1) |

### Etapa 4 — Telas (3 ou 4 agentes em paralelo, arquivos disjuntos)

| Agente | Dono de |
|---|---|
| `Sidebars` | `app/channel_sidebar.rs`, `app/guild_rail.rs`, `app/member_sidebar.rs`, `components/{channel,guild,member}_item.rs` |
| `Chat` | `chat/message_list.rs`, `chat/chat_view.rs`, `chat/reactions.rs` (lista virtualizada; **não** tocar em `markdown.rs`) |
| `Friends` | `app/friends_hub.rs` |
| `Picker` | `chat/media_picker.rs` |

### Etapa 5 — Configurações e voz

| Agente | Dono de | Observação |
|---|---|---|
| `Settings` | `app/user_settings.rs` | Fazer junto com a purga dos dados falsos de Nitro, boosts e conexões |
| `Stage` | `app/voice_call_panel.rs`, `app/header_bar.rs` | Tarefa em `PENDENCIAS_POS_GPUI_KIT.md` seção 6. Só depois dos testes manuais de voz |

### Revisão

Em cada etapa com mais de um agente, vale um agente `Reviewer` (papel `Reviewer`
do Relay) que só lê diffs e roda a suíte. O orquestrador continua auditando,
mas o volume de diff desta migração é grande.

---

## 7. Verificação visual obrigatória: comparar com o Discord por screenshot

Requisito do dono do projeto: **cada agente fica monitorando o Discord e tira
print para ver se ficou realmente igual.** Na primeira rodada isso não
aconteceu de verdade: os agentes inspecionaram o Discord, mas ninguém tirou
screenshot do nosso app, então "igual ao Discord" foi sempre uma afirmação sem
prova.

### 7.1 O que falta para isso funcionar

Não existe hoje uma forma de um agente fotografar o `discord-rs`. É a primeira
coisa a construir (agente `Foundation`). Caminhos a avaliar, nenhum testado:

- Um modo do próprio app que abre uma tela específica num estado determinístico
  e grava um PNG da janela.
- Captura da janela pelo portal de screenshot do sistema (a máquina é Wayland,
  COSMIC). Pode exigir confirmação do usuário a cada captura; se exigir, não
  serve para agente.
- O projeto já tem `src/telemetry/comparator.rs` e gera
  `docs/VISUAL_PARITY_DIFF.md`. Não li o que ele compara de fato; avalie se é
  comparação de imagem real ou só uma tabela descritiva.

O estado determinístico precisa de dados de exemplo **só para a captura**, sem
reintroduzir mocks no caminho de produção. Decida isso antes de delegar.

### 7.2 O ciclo que cada agente de interface deve seguir

1. **Antes de codificar:** abrir a tela no Discord (Chrome compartilhado, aba
   própria, somente leitura), tirar screenshot e medir no DOM: dimensões,
   cores, espaçamentos, tipografia, estados de hover e ativo.
2. **Implementar.**
3. **Fotografar o nosso app** na mesma tela e no mesmo tamanho de janela.
4. **Comparar as duas imagens** e listar cada diferença com a medida.
5. **Corrigir e repetir** até a lista zerar ou sobrar só o que é impossível no
   GPUI, com justificativa.
6. **Entregar as duas imagens** no relatório (caminho dos arquivos), mais a
   lista de diferenças restantes.

### 7.3 Regras

- "Conferi no navegador" sem as duas imagens não vale.
- O orquestrador abre as imagens e confere antes de integrar.
- Telas que só existem dentro de uma chamada, ou que exigem ação na conta, não
  podem ser abertas pelo agente (modo somente leitura). Nesses casos o agente
  diz que não pôde inspecionar; o dono do projeto fornece o screenshot.
- Nenhum screenshot com conversa, nome ou avatar de terceiros entra no
  repositório. Guarde fora do projeto.

---

## 8. Definição de pronto da migração

- `gpui` 0.2.2 fora do `Cargo.toml`; `gpui-kit` em versão exata.
- Nenhum componente morto em `src/components/`.
- Nenhum avatar desenhado fora do componente `Avatar`; decoração visível em todo
  lugar onde há foto de usuário.
- Nenhuma cor fixa nas telas; tudo pelo tema.
- Popout de perfil ancorado, e modal completo só a partir dele.
- Par de screenshots (Discord e discord-rs) para cada tela principal, com as
  diferenças restantes listadas.
- Suíte inteira verde com `--no-fail-fast`, rodada pelo orquestrador.
- Contagem de linhas de `src/app`, `src/chat` e `src/components` menor que as
  29 mil de hoje. Não há meta numérica verificada; meça e relate.
