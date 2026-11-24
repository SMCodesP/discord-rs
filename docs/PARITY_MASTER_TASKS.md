# Discord RS — Master Task Board (Paridade Total com Discord Oficial)

> **Arquivo Vivo Compartilhado** entre o Orquestrador (Antigravity / ORCHTOPUS), o agente **Explorer** (em loop contínuo de exploração e escopo) e a frota de até **24 agentes OpenCode** no MAO.
>
> **Regras de Atualização deste Arquivo:**
> 1. Tarefas concluídas e verificadas (com evidência de `cargo check`, testes e/ou inspeção visual) são marcadas com `[x]` e anotadas com data, commit/layer e resumo do que foi provado.
> 2. **Novas tarefas descobertas** (pelo `Explorer`, testes live, inspeção do Discord Oficial ou bugs encontrados) devem ser adicionadas **sempre lá embaixo**, na seção **Novas Tarefas Descobertas (Backlog Dinâmico)**, com ID sequencial (`T-XXX`), severidade, arquivos suspeitos, evidências (logs/screenshots/DOM/rede) e critério de pronto.
> 3. **Zero Mocks / Zero Dados Sensíveis**: Nunca commitar token, ID real de usuário/servidor, avatar real ou conteúdo de conversa real. Em testes automatizados, usar apenas dados sintéticos.
> 4. **Compilação Segura**: Sempre usar `flock /tmp/discord-rs-cargo-A.lock` ou `flock /tmp/discord-rs-cargo-B.lock` com `-j 4` e testes isolados com `DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/discord-rs-test-bus XDG_CONFIG_HOME=$(mktemp -d)`.

---

## 0. Infraestrutura no Canvas MAO (Portais & Instâncias)

| Portal | Tipo | Conta / Função | Logs / Inspeção |
|---|---|---|---|
| `Discord-Desktop` | App Portal (`discord --remote-debugging-port=9222`) | Referência oficial já logada (`SMCodes`) — DevTools CDP na porta `9222` + screenshots | MCP `chrome-devtools` (`http://127.0.0.1:9222`) + `/tmp/discord-rs-logs/discord-official.log` |
| `DiscordRS-SMCodes` | App Portal (`discord-rs`) | Instância 1 — Conta principal (`~/.config/discord-rs`) | `/tmp/discord-rs-logs/smcodes.log` |
| `DiscordRS-xinxyl` | App Portal (`discord-rs`) | Instância 2 — Conta secundária (`xinxyl_`, XDG isolado) | `/tmp/discord-rs-logs/xinxyl.log` |
| `DiscordRS-Lab` | App Portal (`discord-rs`) | Instância 3 — Testes visuais / telas paralelas / stress | `/tmp/discord-rs-logs/lab.log` |

---

## 1. Frota de Agentes OpenCode (Orquestração Contínua — até 24 simultâneos)

| Slot | Agente | Papel / Frente | Status | Tarefa Atual |
|---|---|---|---|---|
| 01 | `Explorer` | **Loop Contínuo de Exploração, Paridade & Escopo** | Ativo (Loop) | Varrer app, Discord Oficial, logs e código; alimentar seção final |
| 02 | `Voice-Audio` | Auditoria e correção da pilha de áudio/Opus/resampler/mixer (voz bugada) | Ativo | `T-001` |
| 03 | `Voice-Proto` | Auditoria Voice Gateway v8, UDP, RTP, Crypto, DAVE E2EE, Session | Ativo | `T-002` |
| 04 | `Voice-Stage` | UI do Palco de Chamada, anel de fala, seleção de dispositivos, DM ring | Ativo | `T-003` |
| 05 | `QA-Voice` | Agente dedicado de testes de Voz (unitários, integração, análise de logs live) | Ativo | `T-004` |
| 06 | `Chat-Msgs` | Envio/edição/exclusão/reply/fixar/markdown/anexos/embeds/mensagens | Ativo | `T-005` |
| 07 | `QA-Msgs` | Agente dedicado de testes de Mensagens (DM SMCodes <-> xinxyl_, guilda, edge cases) | Ativo | `T-006` |
| 08 | `Profile-Decor` | Perfil popout + modal completo, decorações de avatar em todo o app, efeitos animados | Ativo | `T-007` |
| 09 | `QA-Profile` | Agente dedicado de testes de Perfil, Decorações, Banners, Bio, Quick Message | Ativo | `T-008` |
| 10 | `Media-Gifs` | Picker de GIFs (Tenor/Klipy), Stickers, Emojis customizados, reações | Ativo | `T-009` |
| 11 | `QA-Media` | Agente dedicado de testes de GIFs, Stickers, Emojis e cache de mídia | Ativo | `T-010` |
| 12 | `Design-Shell` | Paridade visual pixel-a-pixel: Guild Rail, Channel Sidebar, Header Bar, User Panel | Ativo | `T-011` |
| 13 | `Design-Chat` | Paridade visual pixel-a-pixel: Message List, Editor, Member Sidebar, Search | Ativo | `T-012` |
| 14 | `Design-Friends` | Paridade visual & funcional: Friends Hub, Active Now, DMs Fixadas, Presença | Ativo | `T-013` |
| 15 | `Design-Settings` | Paridade visual & funcional: User Settings (todas as abas, sem mocks) | Ativo | `T-014` |
| 16 | `QA-Visual` | Agente dedicado de paridade visual (`ui_shot`, comparação com Discord Oficial) | Ativo | `T-015` |
| 17 | `Net-Gateway` | Gateway WebSocket, REST, Rate Limiter, Reconexão, Cache, Protobuf Settings | Ativo | `T-016` |
| 18 | `QA-Net` | Agente dedicado de testes de Rede, Cache, Rate Limit, Startup e Login | Ativo | `T-017` |

*(Slots 19 a 24 reservados para expansão sob demanda conforme o `Explorer` escopar novas frentes críticas ou testes adicionais!)*

---

## 2. Tarefas Iniciais Prioritárias

### P0 — Voz na Call (Bug Crítico Reportado pelo Usuário)
- [x] **T-001** `[Voice-Audio]` — **Diagnosticar e corrigir voz bugada na call (áudio picotado/robótico/mudo/dessincronizado)**
  - **Concluído e integrado na `main` em 2026-11-24** (layer `v-audio`, commit `e5b9d3b`): callbacks `cpal` agora usam anéis SPSC não bloqueantes; captura/playback em workers dedicados com pré-buffer de 60 ms e anel de 250 ms; resampling 44,1/48 kHz corrigido; Opus 48 kHz estéreo/20 ms; mixer com jitter adaptativo, PLC Opus por 200 ms, fades, ganho equal-power e limiter; 33 testes unitários de mídia + 6 `audio_engine_test` verdes. Próximo refinamento: `T-019` (ordenação por sequence/timestamp) + `T-039b` (RMS/ganho real).

- [ ] **T-002** `[Voice-Proto]` — **Auditar e corrigir protocolo de voz ponta a ponta (Voice Gateway v8, UDP, RTP, AEAD Crypto, DAVE E2EE, VoiceSession)**
  - **Escopo**: Auditar `src/protocol/voice_gateway.rs`, `src/protocol/voice_udp.rs`, `src/protocol/voice_crypto.rs`, `src/protocol/voice_dave.rs`, `src/protocol/voice_session.rs`.
  - **Suspeitas iniciais**: Nonce/AAD do `aead_xchacha20_poly1305_rtpsize` ou `aead_aes256_gcm_rtpsize`, extensão de header RTP (RFC 5285 / one-byte header stripping), integração com `davey` (transição de epoch / encrypt & decrypt de frames Opus, passthrough de frames de silêncio `0xF8 0xFF 0xFE`), SSRC mapping para participantes, speaking op 5.
  - **Critério de pronto**: Interoperabilidade RTP + AEAD + DAVE verificada nos mínimos detalhes da spec oficial do Discord Voice v8, testes cobrindo pacotes com/sem RTP header extension e DAVE frames.

- [x] **T-003** `[Voice-Stage]` — **Revisar Palco de Chamada, Controles de Voz/Vídeo, DM Ring e Estado Real**
  - **Concluído e integrado na `main` em 2026-11-24** (layer `v-stage`, commit `ec7ff9b`): estado real de participantes/fala/`self_stream`, sincronização mute/deafen e dispatch único (`T-020`), RTT sem valor fixo, erros visíveis, seletores reais de microfone/saída/câmera, montagem de voz no `RootView`. Testes: `guild_voice` 7/7, `voice_stage_and_parity` 13/13, suíte de voz 71/71, unitários 10/10. Pipeline de transmissão remota de vídeo/tela segue em `T-023`.

- [x] **T-004** `[QA-Voice]` — **Suíte dedicada de testes de Voz & Análise de Logs Live**
  - **Concluído em 2026-11-24** (layer `qa-voice`, commit `22c78af`): suíte existente 77/77 e lib 175/175 passaram; adicionou 10 testes de regressão (4/10 passaram, **6 falharam expondo bugs reais**: IP Discovery sem validação, DAVE sem propagação, `NewSession` sem rebind UDP/chave, heartbeat não positivo aceito). Mantido na layer `qa-voice` até correção em `Voice-Proto`/`Voice-Audio`.

### P1 — Mensagens, Chat e Interações
- [ ] **T-005** `[Chat-Msgs]` — **Revisar ciclo completo de Mensagens (DM e Guilda)**
  - **Escopo**: `src/chat/chat_view.rs`, `src/chat/message_list.rs`, `src/chat/editor.rs`, `src/chat/markdown.rs`, `src/chat/reactions.rs`, `src/app/state/messages.rs`.
  - **Foco**: Envio otimista + confirmação REST/Gateway (sem duplicação!), edição inline, exclusão com confirmação, reply/menção, fixar/desfixar, anexos/imagens, embeds, markdown completo (code blocks, spoilers, quotes, links, timestamps `<t:...>`), scroll infinito / paginação para cima, indicador de digitação (`TYPING_START`).

- [x] **T-006** `[QA-Msgs]` — **Suíte dedicada de testes de Mensagens & Fluxo Multi-Conta**
  - **Concluído em 2026-11-24** (layer `qa-msgs`, commit `a72a2ff`): adicionou **25 novos testes de QA** (11 chat + 14 render/search) reproduzindo **14 defeitos reais**. Suítes originais 56/56, E2E 9/9, `cargo check` OK. Mantido na layer `qa-msgs` para guiar correções.

### P1 — Perfil, Decorações e Efeitos
- [ ] **T-007** `[Profile-Decor]` — **Paridade total de Perfil (Popout + Modal), Decorações de Avatar e Efeitos de Perfil**
  - **Escopo**: `src/app/user_profile_popout.rs`, `src/app/state/profile.rs`, `src/components/avatar.rs`, `src/components/profile_effect.rs`.
  - **Foco**: Popout ancorado fiel ao Discord, transição para modal completo, decorações de avatar visíveis em TODOS os lugares (DM list, chat, member sidebar, voice, friends hub, active now, user panel, popout, modal), efeitos de perfil com animação temporal real (`start`, `duration`, `loop`, `loopDelay`), banners, badges, cargos, notas, envio rápido de mensagem.

- [x] **T-008** `[QA-Profile]` — **Suíte dedicada de testes de Perfil & Decorações**
  - **Concluído em 2026-11-24** (layer `qa-profile`, commit `56d7327`): +16 novos testes (`44/52` passaram, **8 falhas expondo bugs reais**: 6 em propagação/renderização de decoração + 4 pontos de `Avatar` omitindo dados/decoração, e **2 no quick-send onde resposta antiga fecha o popout atual**). Confirmado: nenhum avatar de usuário desenhado fora do componente `Avatar`. Mantido na layer `qa-profile` até correção por `Profile-Decor`.

### P1 — GIFs, Stickers e Emojis
- [ ] **T-009** `[Media-Gifs]` — **Paridade total de GIFs, Stickers, Emojis e Reações**
  - **Escopo**: `src/chat/media_picker.rs`, `src/chat/reactions.rs`, `src/protocol/discord_api.rs` (rotas de GIF/Tenor, stickers, emojis), `src/protocol/media_cache.rs`.
  - **Foco**: Busca e trending de GIFs funcionando de verdade via API, renderização animada/preview de GIFs no picker e no chat, stickers padrão + guilda, emojis unicode + customizados (incluindo animados), favoritos/recentes, envio real ao clicar.

- [x] **T-010** `[QA-Media]` — **Suíte dedicada de testes de GIFs, Stickers, Emojis e Cache de Mídia**
  - **Concluído em 2026-11-24** (layer `qa-media`, commit `555b2c6`): adicionou `tests/media_picker_and_reactions_test.rs` (1066 linhas, 34 testes). `media_cache_test`: 5/5 passaram; nova suíte: 23 passaram e **11 falharam expondo regressões reais** (parsing GIF/Tenor, URLs de stickers, recentes customizados, disponibilidade, cache HTTP/refresh e reações add/remove). Mantido na layer `qa-media` até correção em `T-009`/`T-018` para manter `main` verde.

### P1 — Design & Paridade Visual Pixel-a-Pixel
- [ ] **T-011** `[Design-Shell]` — **Paridade visual: Guild Rail, Channel Sidebar, Header Bar, User Panel**
  - **Escopo**: `src/app/guild_rail.rs`, `src/components/guild_item.rs`, `src/app/channel_sidebar.rs`, `src/app/header_bar.rs`, `src/tokens/*`.
  - **Foco**: Comparar lado a lado com `Discord-Oficial` (DOM/CSS/screenshots): raios de ícones de servidor, pastas, badges de unread/menção, espaçamentos, fontes, cores exatas do tema escuro/claro do Discord, painel de usuário inferior.

- [ ] **T-012** `[Design-Chat]` — **Paridade visual: Message List, Editor, Member Sidebar, Search**
  - **Escopo**: `src/chat/message_list.rs`, `src/chat/editor.rs`, `src/app/member_sidebar.rs`, `src/chat/search.rs`.
  - **Foco**: Agrupamento de mensagens consecutivas, hover actions bar (reagir/responder/editar/mais), cores de cargos, timestamps, divisores de data/não lidas, sidebar de membros agrupada por cargo + offline, painel de busca.
  - **Evidência CDP do Ciclo 2:** `/tmp/opencode/discord-composer-friends-measurements.md` mediu composer **658.5×58/r8**, textarea 56px (máx. 50vh), editor 16px/400/22px com padding `17px 0 17px 10px`, botão `+` efetivo 32×32/r8/ícone 20px e ações mínimas 32×32. O RS usa `+` 24×24/r16/ícone 16px (`src/chat/editor.rs:105-122`) e input sem o gradiente/altura medidos (`src/chat/editor.rs:589-612`).

- [ ] **T-013** `[Design-Friends]` — **Paridade visual & funcional: Friends Hub, Active Now, DMs Fixadas, Presença**
  - **Escopo**: `src/app/friends_hub.rs`, `src/app/state/friends_dms.rs`, `src/protocol/pinned_dms.rs`.
  - **Foco**: Abas Online/Todos/Pendentes/Bloqueados/Adicionar Amigo, cartões "Ativo Agora" com imagens reais de atividade e barra de progresso Spotify, presença (online/idle/dnd/offline/custom status).

- [ ] **T-014** `[Design-Settings]` — **Paridade visual & funcional: User Settings (todas as abas) + Zero Mocks**
  - **Escopo**: `src/app/user_settings/*`, `src/app/state/settings.rs`, `src/app/state/seed.rs`.
  - **Foco**: Garantir 0 dados fictícios em produção, abas fiéis ao Discord, seleção real de dispositivos de áudio/vídeo em Voz e Vídeo, troca de idioma e tema funcionando.

- [x] **T-015** `[QA-Visual]` — **Auditoria Visual Automatizada & Comparação com Discord Oficial**
  - **Concluído em 2026-11-24** (layer `qa-visual`, commit `803dd1e`): suíte base 21/21 verde; adicionou `tests/visual_regression_test.rs` (6 passaram, **3 falharam expondo divergências reais**: `bg_input` `#111214` vs `#383a40`, `blurple hover/active` `#4752c4/#3c45a5` vs `#4452bb/#3a48a3`, `VisualComparator` com cores hardcoded e fallback de fonte marcado como 100%).

- [ ] **T-057** — **P1** — **Design/Tokens** — **Corrigir tokens `bg_input`, `blurple hover/active` e `VisualComparator` expostos por `QA-Visual` (`commit 803dd1e`)**
  - **Arquivos:** `src/tokens/colors.rs`, `src/tokens/kit_theme.rs`, `src/telemetry/comparator.rs`.

- [ ] **T-058** `[QA-Visual]` — **P2** — **QA/Visual Multi-Tema & Fixtures** — **Validar todas as 33 fixtures de `ui_shot`, temas Light/Dark/Midnight, escala de fonte 12–24px e modo Compacto vs Cozy**
  - **Arquivos:** `tests/visual_themes_and_fixtures_test.rs` (novo).
  - **Status:** Em execução por `QA-Visual`.

### P2 — Rede, Gateway, Cache, Auth e Estabilidade
- [ ] **T-016** `[Net-Gateway]` — **Auditoria de Gateway, REST, Rate Limiter, Reconexão, Cache e Protobuf**
  - **Escopo**: `src/protocol/discord_api.rs`, `src/protocol/client_manager.rs`, `src/protocol/rate_limit/*`, `src/protocol/cache/*`, `src/protocol/user_settings_proto.rs`, `src/app/state/gateway_events.rs`, `src/app/state/connection.rs`.
  - **Foco**: Reconexão robusta (RESUME vs IDENTIFY), tratamento de 429/rate-limit buckets, evitar race conditions, zero vazamentos de memória, logs estruturados sem segredos.

- [x] **T-017** `[QA-Net]` — **Suíte dedicada de testes de Rede, Cache, Rate Limit, Auth e E2E**
  - **Concluído e integrado na `main` em 2026-11-24** (layer `qa-net`, commit `6414b3f`): baseline 50/50 + endurecimento offline de `tests/cache_and_ratelimit_test.rs` (diretórios isolados, bucket normalizado, 429 `Retry-After`, limite global, cache corrompido) e `tests/protocol_tests.rs` (servidor HTTP real em loopback para queda de conexão e 429). Seam de Gateway fica para `T-050` após land de `Net-Gateway`.

---

## 3. Novas Tarefas Descobertas (Backlog Dinâmico — Explorer Loop & QA)

> **Instrução para o `Explorer` e agentes QA:** Sempre acrescentem novas descobertas **aqui embaixo** com o próximo ID (`T-018`, `T-019`, ...), incluindo:
> - **Prioridade** (`P0` crítico / `P1` paridade principal / `P2` polimento / `P3` melhoria futura)
> - **Área** (Voz, Chat, Perfil, Mídia, Design, Rede, Settings, UX)
> - **Evidência** (arquivo:linha, log, screenshot ou comportamento comparado com o Discord Oficial)
> - **O que falta / Como corrigir**

### Ciclo 1 — Auditoria profunda de 2026-11-24

**Linha de base verificada:** `cargo check -j 4` concluiu sem erros; suíte isolada com `DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/discord-rs-test-bus` e `XDG_CONFIG_HOME` temporário executou 34 binários/doc-tests, com **451 testes passando e 0 falhando**. Os testes existentes validam contratos locais, mas não cobrem vários dos fluxos reais abaixo.

**Fontes de evidência visual/log:**
- `/tmp/discord-oficial.png` e snapshot/DOM do portal web oficial em `https://discord.com/login`, viewport 1280×800: campo de e-mail 414×44 em `(281,333)`, botão `Log In` 416×40 em `(280,508)`, QR 176×176, fonte `gg sans` 16px. O portal web foi substituído pelo orquestrador durante a auditoria; nenhum login ou alteração de conta foi feita.
- `/tmp/discord-desktop.png`, cliente oficial autenticado em Friends, tela 1272×756: botão `Add Friend` separado no canto superior direito, abas Online/All/Pending e sem painel Active Now nessa largura/estado. Inspeção somente leitura, sem mensagens, amizades, configurações ou chamadas.
- `/tmp/discordrs-smcodes.png`, `/tmp/discordrs-xinxyl.png`, `/tmp/discordrs-lab.png` e `/tmp/discordrs-friends.png`: instâncias nativas; a principal exibiu iniciais no trilho de servidores e as outras duas permaneceram no login.
- `/tmp/discord-rs-logs/smcodes.log:51-64`: 14 erros consecutivos `404` do CDN para ícones de guilda. `/tmp/discord-rs-logs/*.log:15`: portal desktop indisponível no ambiente. Não havia chamada real nem eventos RTP/DAVE nos logs atuais.

- [ ] **T-018** — **P0** — **Voz/Protocolo** — **Renegociar mídia após reconnect/resume do Voice Gateway**
  - **Arquivos:** `src/protocol/voice_session.rs:1216-1220`, `src/protocol/voice_session.rs:1243-1247`, `src/protocol/voice_session.rs:1301-1314`, `src/protocol/voice_session.rs:1447-1472`, `src/protocol/voice_gateway.rs:1391-1394`, `src/protocol/voice_gateway.rs:1419-1421`, `src/protocol/voice_udp.rs:503-510`.
  - **Evidência:** o socket UDP, `RtpCrypto`, `local_ssrc`, sequência e timestamp são criados uma única vez. Quando o gateway emite `Ready`/`Resumed` ou decide `NewSession`, `VoiceSession` apenas volta o estado para `Connected`; não refaz IP Discovery, Select Protocol, Session Description nem atualiza SSRC/chave. Em `NewSession`, o servidor pode atribuir novo SSRC e chave, deixando o transmissor com identidade obsoleta.
  - **O que falta/corrigir:** tornar o handshake de mídia reentrante e orientado por geração de sessão; reconstruir transporte após `Ready`/`SessionDescription` novo, preservando buffers apenas quando o protocolo permitir resume.
  - **Critério de pronto:** teste determinístico injeta fechamento resumível e `4006/4009` após Ready, verifica novo SSRC/chave/IP/porta/Select Protocol e comprova que nenhum pacote é enviado com metadados da sessão anterior; reconexão real em canal de teste é observada pelo dono.

- [ ] **T-019** — **P0** — **Voz/Áudio** — **Implementar jitter buffer, reordenação e PLC por SSRC**
  - **Arquivos:** `src/protocol/voice_session.rs:1261-1288`, `src/protocol/voice_session.rs:435-436`, `src/media/mixer.rs:37-75`, `src/media/mixer.rs:80-108`, `src/media/codec.rs:73-82`.
  - **Evidência:** `sequence` e `timestamp` chegam ao bridge e são descartados (`..` no match). O mixer decodifica e enfileira imediatamente, sem ordenação, deduplicação, detecção de lacuna ou alvo de jitter; underrun vira silêncio e overflow descarta o quadro mais antigo. `decode_loss()` existe, mas não é chamado.
  - **O que falta/corrigir:** buffer por SSRC com ordenação por sequência/timestamp, tolerância a wrap, política adaptativa de atraso, PLC em lacunas e métricas de perda/jitter.
  - **Critério de pronto:** testes com fora de ordem, duplicado, perda isolada, burst loss e wrap de `u16/u32` produzem ordem correta, sem duplicação audível, com PLC acionado; teste de carga mantém latência dentro do orçamento documentado.

- [x] **T-020** — **P1** — **Voz/Estado** — **Corrigir semântica deafen→mute e eliminar dispatch duplicado**
  - **Concluído em 2026-11-24** junto com `T-003` (`Voice-Stage`, commit `ec7ff9b`, 71/71 testes de voz verdes).
  - **Arquivos:** `src/protocol/voice_session.rs:806-831`, `src/media/audio.rs:104-121`, `src/app/state/voice.rs:410-471`.
  - **Evidência:** `set_deafened(true)` silencia apenas a saída e envia Opcode 4 com `self_mute` atual, que pode ser `false`; o capturador continua transmitindo. `AppState` corrige `current_user.is_muted`, mas chama apenas `set_deafened`. Além disso, mute/deafen/device switch são despachados duas vezes: uma pela sessão guardada na chamada e outra pelo `ClientManager`, que retorna a mesma sessão.
  - **O que falta/corrigir:** tornar `VoiceSession` a única autoridade; deafen deve implicar mute local e remoto, e undeafen deve restaurar o estado anterior de mute; remover caminhos duplicados de `AppState`.
  - **Critério de pronto:** um clique gera exatamente um comando de áudio e um Opcode 4; testes cobrem deafen a partir de unmuted, undeafen e falha de sinalização sem estado divergente.

- [ ] **T-021** — **P1** — **Voz/DAVE** — **Falhar fechado quando DAVE negociado não fica pronto**
  - **Arquivos:** `src/protocol/voice_session.rs:1193-1203`, `src/protocol/voice_session.rs:1246-1248`, `src/protocol/voice_dave.rs:233-263`, `src/protocol/voice_dave.rs:466-510`.
  - **Evidência:** erro de `init_session` apenas gera `warn` e a sessão continua; `encrypt_opus_frame`/`decrypt_opus_frame` retornam passthrough enquanto MLS não está pronto, sem prazo máximo. O estado vira `Connected` antes de qualquer garantia de E2EE.
  - **O que falta/corrigir:** exigir readiness DAVE quando `dave_protocol_version > 0`, limitar passthrough à janela oficial de grace e expor erro fatal/reação de recuperação quando o handshake MLS falhar.
  - **Critério de pronto:** testes cobrem versão 0, versão 1 pronta, external-sender inválido, proposal/welcome inválido e timeout; áudio só flui em passthrough quando permitido e a UI nunca mostra “Conectado” sem o estado criptográfico real.

- [ ] **T-022** — **P1** — **Voz/Hardware** — **Não reportar chamada conectada com bridge de áudio no-op**
  - **Arquivos:** `src/protocol/client_manager.rs:696-740`, `src/protocol/voice_session.rs:1225-1248`, `src/app/state/voice.rs:333-380`.
  - **Evidência:** em produção, ausência de microfone ou saída cria um bridge falso que responde `Ok(())` a Start/Switch e descarta pacotes recebidos. A sessão então marca `Connected`, embora não capture nem reproduza áudio. O mesmo trecho de `AppState` também pode trocar o dispositivo duas vezes.
  - **O que falta/corrigir:** retornar erro explícito e acionável quando hardware obrigatório não existe; distinguir modo de teste de produção; centralizar a troca de dispositivos.
  - **Critério de pronto:** teste com enumeração vazia verifica estado `Disconnected`, mensagem específica e nenhuma confirmação de sucesso; com hardware sintético, switch é executado uma única vez e falha preserva o dispositivo anterior.

- [ ] **T-023** — **P1** — **Voz/Vídeo** — **Implementar pipeline real de vídeo e compartilhamento de tela**
  - **Arquivos:** `src/app/state/voice.rs:383-393`, `src/app/voice_call_panel.rs:740-821`, `src/app/header_bar.rs:398-408`, `src/protocol/voice_udp.rs:171-178`, `src/capture/camera.rs:160-212`, `src/capture/screen.rs:375-405`.
  - **Evidência:** câmera e tela são botões desabilitados; `toggle_camera`/`toggle_screen_share` apenas registram `warn`. A captura local existe, mas o transporte só monta RTP de áudio Opus e não há codificação/decodificação de vídeo remoto.
  - **O que falta/corrigir:** negociar codec/resolução, empacotar RTP de vídeo, integrar captura local, decodificar/renderizar remoto, tratar SSRC de vídeo e implementar início/parada de tela com seleção real de janela/monitor.
  - **Critério de pronto:** teste de integração sintético valida payload types, timestamps e troca de fonte; chamada real mostra preview local e quadro remoto, e compartilhamento aparece/desaparece após confirmação do servidor, sem tile fictício.

- [ ] **T-024** — **P1** — **Chat/Mensagens** — **Preservar e enviar `message_reference` nas respostas**
  - **Arquivos:** `src/chat/chat_view.rs:416-432`, `src/chat/chat_view.rs:482-519`, `src/app/state/messages.rs:166-213`, `src/protocol/discord_api.rs:1018-1034`, `src/domain/message.rs:183-218`.
  - **Evidência:** o callback recebe `_reply_id` e o ignora; `send_message` não aceita referência; o corpo REST contém apenas `{ "content": ... }`. O banner de resposta some no envio, mas a mensagem enviada é uma mensagem comum, sem vínculo com a original.
  - **O que falta/corrigir:** carregar `message_reference`, `referenced_message`, flag de menção e autor; enviar `message_reference.message_id` + `allowed_mentions`; renderizar preview clicável da resposta.
  - **Critério de pronto:** teste de contrato REST exige `message_reference` e menção configurável; teste de UI verifica preview, cancelamento, Editar/Escape e limpeza somente após sucesso.

- [ ] **T-025** — **P1** — **Chat/Reações** — **Manter identidade de emoji customizado e alternar reação via API**
  - **Arquivos:** `src/protocol/discord_api.rs:2462-2472`, `src/domain/message.rs:96-114`, `src/chat/reactions.rs:61-86`, `src/chat/chat_view.rs:437-445`, `src/app/state/messages.rs:25-40`, `src/protocol/discord_api.rs:1050-1099`.
  - **Evidência:** o parser guarda apenas `emoji.name`, perde `id`/`animated` e inventa `"👍"` se ausente. O badge renderiza esse nome como texto. Clicar sempre incrementa localmente e nunca chama PUT/DELETE, portanto reações customizadas não funcionam e o contador não reflete remoção.
  - **O que falta/corrigir:** modelo estruturado `{id,name,animated}`; URL CDN correta; chave de reação `name:id`; toggle otimista com rollback e reconciliação Gateway.
  - **Critério de pronto:** testes cobrem unicode, customizado estático e animado, primeira reação, remoção, contador em zero e erro HTTP; snapshot comprova imagem do emoji, não texto.

- [ ] **T-026** — **P1** — **Chat/Mensagens** — **Fechar ciclo de vida do envio otimista**
  - **Arquivos:** `src/app/state/messages.rs:177-212`, `src/app/state/messages.rs:93-118`, `src/app/state/gateway_events.rs:123-156`, `src/chat/message_list.rs:883-907`, `src/protocol/transport.rs:60-75`.
  - **Evidência:** falha no envio só produz `log::warn`; a mensagem `temp-*` permanece `is_pending=true` para sempre. A reconciliação usa conteúdo+autor, podendo substituir a mensagem errada quando duas idênticas são enviadas. Não existe evento de falha de envio nem UI de retry.
  - **O que falta/corrigir:** nonce/client ID temporário, correlação com a resposta REST/Gateway, timeout, estado `failed`, retry e rollback/delete explícito; deduplicação não pode depender apenas do texto.
  - **Critério de pronto:** testes de sucesso, atraso, duas mensagens idênticas, timeout e 429/falha mostram exatamente uma mensagem confirmada ou uma falha recuperável, sem pendência infinita.

- [ ] **T-027** — **P1** — **Chat/Anexos** — **Renderizar anexos reais e implementar upload**
  - **Arquivos:** `src/chat/message_list.rs:605-697`, `src/chat/editor.rs:105-122`, `src/chat/chat_view.rs:1795-1808`, `src/protocol/discord_api.rs:1018-1034`.
  - **Evidência:** imagens recebidas viram uma caixa com `🖼️ nome` e não usam `att.url`; arquivos são cards sem ação. O botão `+` não recebe `on_attachment` no `ChatComposer`. O envio é JSON simples, sem multipart nem endpoint de upload.
  - **O que falta/corrigir:** file picker, drag-and-drop e paste de imagem; preview/remover; upload multipart oficial; renderizar imagem/GIF/vídeo e card de arquivo com abrir/baixar/copiar link; respeitar limites reais da conta.
  - **Critério de pronto:** teste sintético de multipart valida campos/headers; UI exibe mídia recebida e arquivo; falha de upload preserva o anexo local e não insere mensagem confirmada.

- [ ] **T-028** — **P1** — **Mídia/GIFs** — **Conectar o picker de GIFs à API real**
  - **Arquivos:** `src/chat/chat_view.rs:804-822`, `src/chat/chat_view.rs:849-856`, `src/chat/media_picker.rs:1823-1875`, `src/protocol/media_cache.rs:1057-1075`, `src/protocol/discord_api.rs:457-539`.
  - **Evidência:** `MediaPickerConfig.gifs` recebe `get_curated_tenor_gifs_for_category`, que retorna `Vec::new()`. O callback de busca só altera uma string. Os métodos `get_trending_gifs`/`search_gifs` existem, mas não são chamados pela UI; portanto a aba sempre mostra “Nenhum GIF disponível”.
  - **O que falta/corrigir:** estado assíncrono de loading/erro/vazio, debounce, cache por query/categoria, chamada real de trending/search, preview animado e envio do URL confirmado.
  - **Critério de pronto:** teste com HTTP sintético valida query, locale, media_format, cache e resposta vazia; teste de UI comprova resultados, preview, seleção e uma única mensagem enviada.

- [ ] **T-029** — **P1** — **Mídia/Stickers** — **Carregar pacotes oficiais e permitir favoritos**
  - **Arquivos:** `src/protocol/discord_api.rs:425-453`, `src/chat/chat_view.rs:834-856`, `src/chat/media_picker.rs:1393-1417`, `src/protocol/media_cache.rs:710-743`.
  - **Evidência:** `get_sticker_packs()` nunca é usado pelo picker; `get_default_sticker_packs()` retorna vazio. A seção “Favoritos” lê o cache, mas nenhum controle chama `add_favorite_sticker`/`remove_favorite_sticker` fora de testes. Só stickers de guilda já carregadas aparecem.
  - **O que falta/corrigir:** buscar `/sticker-packs` sob demanda, cachear, exibir packs, favoritar/desfavoritar por contexto e filtrar `available`/permissões.
  - **Critério de pronto:** teste de API sintética valida packs e stickers; favoritos persistem após reinício; sticker indisponível não é enviável; clique envia `sticker_ids` e aguarda confirmação.

- [ ] **T-030** — **P1** — **Mídia/Soundboard** — **Enviar sons reais em vez de texto descritivo**
  - **Arquivos:** `src/chat/chat_view.rs:779-802`, `src/chat/chat_view.rs:849-856`, `src/chat/media_picker.rs:2006-2057`, `src/protocol/discord_api.rs:542-574`.
  - **Evidência:** `sounds` é sempre `Vec::new()`; preview apenas registra `log::info`; “enviar” transforma o som em `[Som da Soundboard: ...]` e chama `send_message`, criando uma mensagem de texto que não reproduz áudio.
  - **O que falta/corrigir:** listar sons padrão e da guilda, tocar preview local, usar o endpoint oficial de envio de soundboard e tratar permissões/erros sem texto substituto.
  - **Critério de pronto:** teste de contrato valida payload de soundboard; UI lista sons reais, toca preview e envia som; nenhuma mensagem textual é criada como fallback.

- [ ] **T-031** — **P1** — **Chat/Histórico** — **Implementar paginação infinita para mensagens antigas**
  - **Arquivos:** `src/app/state/guilds_channels.rs:309-323`, `src/protocol/discord_api.rs:952-1015`, `src/chat/chat_view.rs:382-391`.
  - **Evidência:** ao abrir um canal vazio, o app busca apenas as 50 últimas mensagens e nunca usa `before`. `ChatView` não tem gatilho de topo; qualquer mudança de contagem reseta a lista para o fim. Histórico mais antigo é inacessível.
  - **O que falta/corrigir:** cursor `before`, estado `has_more/loading`, deduplicação por ID, preservação de scroll e merge cronológico; suportar `around` para busca.
  - **Critério de pronto:** fixture sintética com 150 mensagens comprova três páginas sem duplicatas, spinner único, preservação da posição e comportamento correto em canal/DM.

- [ ] **T-032** — **P1** — **Chat/Busca** — **Buscar e destacar mensagem ao pular de resultado**
  - **Arquivos:** `src/app/state/search.rs:296-307`, `src/chat/chat_view.rs:602-612`, `src/app/state/guilds_channels.rs:309-323`, `src/protocol/discord_api.rs:952-966`.
  - **Evidência:** `jump_to_message` apenas troca canal/guilda e grava `target_message_id`. O renderer só chama `scroll_to_reveal_item` se a mensagem já estiver entre as 50 carregadas; não há busca `around` nem destaque visual temporário.
  - **O que falta/corrigir:** resolver canal, buscar janela ao redor do ID, aguardar carregamento, destacar e limpar o destaque; tratar mensagem removida/sem permissão.
  - **Critério de pronto:** teste com resultado fora do cache verifica uma requisição `around`, scroll após chegada dos dados, highlight temporário e erro honesto quando a mensagem não existe.

- [ ] **T-033** — **P1** — **Chat/Header** — **Ligar Notificações, Pins, Inbox e Ajuda**
  - **Arquivos:** `src/app/header_bar.rs:409-485`, `src/protocol/discord_api.rs:3047-3062`, `src/protocol/transport.rs:329-339`.
  - **Evidência:** os quatro botões são renderizados sem `on_click`. Existe PUT/DELETE de pin, mas não há GET da lista de pins nem painel de mensagens fixadas. Inbox/Help/notificações não executam ação.
  - **O que falta/corrigir:** implementar `GET /channels/{id}/pins`, painel com jump/unpin, estados de permissão; ligar inbox/notificações/ajuda a fluxos reais ou remover controles não implementados.
  - **Critério de pronto:** testes de API e UI validam lista vazia/cheia, jump, pin/unpin e erro; nenhum botão visível permanece sem handler.

- [ ] **T-034** — **P1** — **UX/Menus** — **Concluir controles inertes e paridade estrutural do Friends Hub**
  - **Arquivos:** `src/app/channel_sidebar.rs:166-193`, `src/app/channel_sidebar.rs:242-255`, `src/app/channel_sidebar.rs:512-519`, `src/app/channel_sidebar.rs:772-783`, `src/app/member_sidebar.rs:332-343`, `src/app/friends_hub.rs:66-75`, `src/app/friends_hub.rs:298-317`, `src/app/friends_hub.rs:385-402`, `src/app/friends_hub.rs:1316-1327`, `src/app/friends_hub.rs:1465-1508`.
  - **Evidência:** busca de DM, Nitro, Loja, fechar DM, itens do menu de canal, “Mencionar”, ícones do header Friends, menu “Mais” e descobrir servidores não têm ação. “Mensagem” no menu de membro abre perfil, não DM. No oficial desktop 1272×756, `Add Friend` é botão separado à direita e Active Now não aparece sem conteúdo; no RS, Add Friend fica dentro da tab bar e o painel de 360px é sempre montado. Medição CDP adicional em `/tmp/opencode/discord-composer-friends-measurements.md`: header 49px; tabs 32px com item `padding 4px 12px`, margem 8px e r8; `Add Friend` 99.75×32/r8; busca 634.5×40; linha de amigo 61px; avatar 32px/margem 12px; nome 16px/600/20px; subtexto 12px/500/15px; ações 36×36.
  - **O que falta/corrigir:** ligar cada ação ao estado/API correto, adicionar estados pendentes/erro e ajustar layout responsivo do Friends Hub medido no cliente oficial.
  - **Critério de pronto:** matriz de testes cobre todos os controles; nenhum item de menu visível é no-op; screenshots em 1280×800 e largura maior/menor comprovam posição de Add Friend e comportamento responsivo do Active Now.

- [ ] **T-035** — **P2** — **UX/Atalhos** — **Implementar atalhos anunciados e seleção de mensagem**
  - **Arquivos:** `src/app/user_settings/keybinds.rs:58-101`, `src/chat/chat_view.rs:1812-1868`, `src/chat/message_list.rs:869-895`.
  - **Evidência:** Settings anuncia `CTRL /`, `E` e `BACKSPACE`, mas o handler só trata `Ctrl+F`, Escape e ArrowUp. Não existe estado de mensagem focada/selecionada, portanto E/Backspace não podem editar/excluir a mensagem correta.
  - **O que falta/corrigir:** foco/seleção acessível, overlay de atalhos, edição/exclusão da mensagem selecionada, navegação por teclado e persistência de rebinds.
  - **Critério de pronto:** testes de key events cobrem todos os atalhos anunciados, foco visível, confirmação de delete e conflito com editor; lista de Settings corresponde ao comportamento real.

- [ ] **T-036** — **P1** — **Design/Mídia** — **Corrigir ícones de guilda com 404 e URLs animadas**
  - **Arquivos:** `src/protocol/discord_api.rs:825-837`, `src/protocol/discord_api.rs:878-887`, `src/app/state/models.rs:197-208`, `src/components/guild_item.rs:231-245`, `src/components/guild_item.rs:567-576`.
  - **Evidência:** `/tmp/discord-rs-logs/smcodes.log:51-64` registra 14 `404` seguidos; a captura da instância principal mostra iniciais no trilho. A construção usa sempre `.png`, inclusive hashes `a_` (animados), e o modelo pode guardar a URL completa também como `icon_hash`. Não há tratamento explícito de falha de CDN.
  - **O que falta/corrigir:** reproduzir com fixture sintética, validar host/caminho/query contra a resposta real, usar `.gif` para `a_`, normalizar `icon` vs `icon_hash` e aplicar fallback visual estável sem imagem quebrada.
  - **Critério de pronto:** teste de regressão cobre ícone estático, animado, ausente e 404; captura com imagem local prova raio/clipping em repouso, hover, selecionado, pasta fechada e aberta; chamada real não emite mais 404.

- [ ] **T-037** — **P2** — **Perfil/Decorações** — **Fechar decoração de avatar em todas as superfícies**
  - **Arquivos:** `src/app/friends_hub.rs:563-568`, `src/app/friends_hub.rs:1619-1624`, `src/app/state/models.rs:250-265`, `src/app/state/models.rs:276-322`, `src/app/root_view.rs:1495-1523`, `src/app/root_view.rs:1663-1688`.
  - **Evidência:** `ActiveNowItem` já carrega `avatar_decoration_data`, mas o avatar do card não recebe `.opt_decoration`. `MessageRequestItem` não possui o campo. Os seletores de amigos para DM em grupo criam `Avatar` apenas com nome/tamanho, sem URL, status ou decoração.
  - **O que falta/corrigir:** propagar decoração/URL/status pelo modelo de message request e pelos diálogos de grupo; padronizar todos os pontos de avatar no componente `Avatar`.
  - **Critério de pronto:** teste de propagação cobre message request, Active Now, add-friend-to-group e create-group; inventário automatizado garante que todo `Avatar` de usuário recebe decoração quando disponível.

- [ ] **T-038** — **P1** — **Settings** — **Tornar configurações reais, persistidas e corretamente mapeadas**
  - **Arquivos:** `src/app/user_settings/mod.rs:88-120`, `src/app/user_settings/mod.rs:297-319`, `src/app/user_settings/profiles.rs:47-105`, `src/app/user_settings/privacy.rs:92-120`, `src/app/user_settings/notifications.rs:15-20`, `src/app/user_settings/notifications.rs:64-137`, `src/app/user_settings/language.rs:76-80`, `src/protocol/discord_api.rs:354-356`.
  - **Evidência:** defaults de volume, Krisp, notificações e idioma são hardcoded e não vêm de `/users/@me/settings`. “Sair da Conta” só fecha o modal. Nome/bio são `div` estáticos, sem edição/save. Privacy usa `allow_dm_from_members` para Sponsored Content e `allow_friend_requests_from_members` para Clips. Notifications mapeia rótulos de streaming/aniversário/online/evento/perfil a campos de badge/flash/sons. Idioma só altera estado do modal.
  - **O que falta/corrigir:** hidratar do servidor, separar campos, editar/salvar com confirmação, persistir preferências locais, aplicar idioma e implementar logout real com limpeza segura do token.
  - **Critério de pronto:** testes de serialização e UI validam cada toggle contra seu campo; save só confirma após resposta; reload preserva valores; logout encerra gateway e remove credencial; nenhum default inventado é exibido como dado da conta.

- [ ] **T-039** — **P1** — **Settings/Voz** — **Ligar dispositivos, volumes e teste de microfone reais**
  - **Arquivos:** `src/app/user_settings/voice_video.rs:33-82`, `src/app/user_settings/voice_video.rs:89-121`, `src/app/user_settings/voice_video.rs:126-168`, `src/app/user_settings/voice_video.rs:174-197`.
  - **Evidência:** os seletores exibem `"Default"` estático e não abrem lista. Sliders só mudam campos do modal. “Mic Test” alterna um bool e a barra usa largura fixa de 180px quando ligada, sem captura/RMS. Perfis de entrada são cards sem ação.
  - **O que falta/corrigir:** enumerar dispositivos reais, aplicar input/output gain no motor, teste de mic com nível real e parada explícita, persistir escolhas e implementar perfis de processamento ou removê-los.
  - **Critério de pronto:** teste com dispositivos sintéticos valida seleção/troca; teste de hardware captura RMS real; falha de dispositivo aparece sem sucesso fabricado; volumes afetam o áudio da chamada.

- [ ] **T-040** — **P1** — **Auth/Login** — **Alinhar login oficial e não persistir token antes do sucesso**
  - **Arquivos:** `src/app/root_view.rs:777-1009`, `src/app/state/connection.rs:8-52`, `src/protocol/auth.rs:25-81`, `src/protocol/auth.rs:157-190`, `src/protocol/bridge.rs:90-150`.
  - **Evidência:** o RS mostra um cartão de 520px com token de bot/usuário; o oficial web inspecionado tem e-mail/senha, QR 176×176, passkey e botão 416×40, com `gg sans` 16px. `authenticate` salva o token em disco antes de o Gateway confirmar autenticação, podendo substituir uma credencial válida por uma inválida.
  - **O que falta/corrigir:** implementar OAuth2 Authorization Code + PKCE/QR oficial (e passkey quando viável), manter token de bot apenas em modo explícito de desenvolvimento, persistir somente após sucesso e renovar/limpar com segurança.
  - **Critério de pronto:** fluxo sintético valida state/nonce/PKCE, cancelamento, expiração e troca de código; token inválido não sobrescreve o anterior; screenshots de login seguem dimensões/fonte do oficial e não expõem segredo.

- [ ] **T-041** — **P2** — **Settings/Acessibilidade** — **Aplicar tema, fonte, modo de mensagem e movimento reduzido**
  - **Arquivos:** `src/main.rs:30-33`, `src/tokens/kit_theme.rs:285-310`, `src/app/state/settings.rs:82-110`, `src/app/user_settings/appearance.rs:259-263`, `src/app/user_settings/appearance.rs:385-389`, `src/chat/message_list.rs:1206-1280`, `src/app/user_profile_popout.rs:450-461`, `src/app/user_profile_popout.rs:1007-1019`.
  - **Evidência:** o tema escuro é instalado uma vez na inicialização; setters de tema/fonte/Cozy-Compact/reduced motion só alteram estado. `AccountThemeUpdated` também não reaplica o tema. O popout usa `render_profile_effect_for_user` (sempre animado), enquanto só o modal respeita reduced motion.
  - **O que falta/corrigir:** reaplicar paleta/tipografia do kit, escalar fonte no chat, alternar layout Cozy/Compact, respeitar reduced motion em todo animação e persistir/sincronizar preferências.
  - **Critério de pronto:** testes comprovam mudança imediata de Dark/Light/Midnight, fonte 12–24px, agrupamento Compact e efeito estático; reload e tema da conta são aplicados sem reiniciar o app.

- [ ] **T-042** — **P1** — **Privacidade/Logs** — **Redigir identificadores pessoais dos logs**
  - **Arquivos:** `src/protocol/client_manager.rs:178`, `src/protocol/client_manager.rs:225`, `src/protocol/client_manager.rs:266`, `src/protocol/client_manager.rs:285`, `src/protocol/bridge.rs:150-201`, `src/protocol/cache_engine.rs:222-249`, `src/app/state/gateway_events.rs:241-242`, `src/protocol/discord_api.rs:958`, `src/protocol/discord_api.rs:1024`.
  - **Evidência:** os logs atuais contêm nome de usuário, Snowflakes de usuário/guilda/canal/mensagem e nomes de servidor. Isso contraria a política de não vazar dados da conta e torna relatórios/diagnósticos perigosos. A divisão `VoiceDiag` já redige alguns campos, mas o restante da pilha não.
  - **O que falta/corrigir:** helper estruturado de redação, IDs sintéticos/parciais, remover conteúdo e nomes de contas, mascarar URLs com IDs e garantir que erros HTTP não incluam payload sensível.
  - **Critério de pronto:** teste com sentinels sintéticos varre todos os logs e falha se encontrar Snowflake, username, nome de servidor, conteúdo ou token; relatórios de suporte podem ser gerados sem dado pessoal.

- [ ] **T-043** — **P1** — **Mídia/GIFs/Reações (Fase 2)** — **Corrigir 11 regressões reais de GIFs/Tenor, Stickers, Emojis, Cache HTTP e Reações + ligar `chat_view.rs` à API real**
  - **Arquivos:** `src/chat/chat_view.rs`, `src/app/state/messages.rs`, `src/app/state/gateway_events.rs`, `src/protocol/client_manager.rs`, `src/protocol/transport.rs`, `src/protocol/discord_api.rs`, `src/chat/media_picker.rs`, `src/chat/reactions.rs`, `src/protocol/media_cache.rs`.
  - **Evidência:** `Media-Gifs` (`hub-task-2914`) + `QA-Media` (`commit 555b2c6`, `tests/media_picker_and_reactions_test.rs` com 11 falhas reais).
  - **Critério de pronto:** os 34/34 testes de `tests/media_picker_and_reactions_test.rs` passam e `chat_view.rs` busca GIFs trending/search e stickers reais.

- [x] **T-044** `[QA-Media]` — **P1** — **QA/Mídia & Chat** — **Suíte dedicada de testes para Anexos, Imagens, Embeds Ricos e Markdown de Mídia**
  - **Concluído em 2026-11-24** (layer `qa-media`, commit `37e8b5a`): adicionou `tests/attachments_embeds_markdown_test.rs` com 28 testes isolados (15 passaram, **13 falharam expondo gaps reais**: spoiler em anexos, schema de embeds, cache `MESSAGE_UPDATE`, timestamps, menções fabricadas/email, links, escapes, code blocks e emoji Unicode jumbo).

- [ ] **T-048** — **P1** — **Chat/Mídia** — **Corrigir 13 gaps reais de Anexos, Embeds e Markdown expostos por `QA-Media`**
  - **Arquivos:** `src/chat/markdown.rs`, `src/chat/message_list.rs`, `src/domain/message.rs`, `src/protocol/discord_api.rs`, `src/app/state/gateway_events.rs`.
  - **Evidência:** `QA-Media` (`commit 37e8b5a`, `tests/attachments_embeds_markdown_test.rs`, 13 falhas reais).
  - **Critério de pronto:** 28/28 testes de `tests/attachments_embeds_markdown_test.rs` passando.

- [x] **T-049** `[QA-Media]` — **P1** — **QA/Notificações & Presença** — **Suíte de regressão para Notificações, Badges de Unread/Menção, Contadores e Presença em Tempo Real**
  - **Concluído em 2026-11-24** (layer `qa-media`, commit `2e47289`): `tests/notifications_badges_and_presence_test.rs` com 24 testes isolados (13 passaram, **11 falharam expondo gaps reais**: unread/mention/ack, agregados de guilda, presença fabricada no Friends Hub, custom status emoji-only, `created_at` em ms, classificação Spotify e defaults/labels de presença).

- [ ] **T-055** — **P1** — **Notificações/Presença** — **Corrigir 11 gaps reais de Unread/Mention/Ack, Agregados de Guilda, Presença e Spotify expostos por `QA-Media`**
  - **Arquivos:** `src/app/state/gateway_events.rs`, `src/app/state/friends_dms.rs`, `src/app/friends_hub.rs`, `src/app/guild_rail.rs`, `src/protocol/discord_api.rs`.
  - **Evidência:** `QA-Media` (`commit 2e47289`, `tests/notifications_badges_and_presence_test.rs`, 11 falhas reais).

- [x] **T-056** `[QA-Media]` — **P2** — **QA/Canais & Permissões** — **Suíte de regressão para Categorias, Canais de Texto/Voz/Anúncio/Fórum/Threads e Permissões de Canal**
  - **Concluído em 2026-11-24** (layer `qa-media`, commit `a895231`): `tests/channels_permissions_and_threads_test.rs` com 18 testes (7 passaram, **11 falharam expondo gaps reais**: tipos modernos/threads, `parent_id` no serde, `permission_overwrites` descartados, ordenação por empate, `CHANNEL_CREATE` ignorado e eventos `CHANNEL_UPDATE`/`DELETE`/`THREAD_*` ausentes).

- [ ] **T-060** — **P2** — **Protocolo/Canais & Threads** — **Corrigir 11 gaps reais de Canais, Permissões e Threads expostos por `QA-Media` (`commit a895231`)**
  - **Arquivos:** `src/domain/channel.rs`, `src/protocol/discord_api.rs`, `src/app/state/gateway_events.rs`, `src/app/state/guilds_channels.rs`.

- [ ] **T-061** `[QA-Media]` — **P2** — **QA/Pastas & Member List Esparsa** — **Suíte de regressão para Guild Folders, ordenação de servidores, Member List esparsa (`op 14` ranges) e cores de cargo (`op 8`)**
  - **Arquivos:** `tests/guild_folders_and_member_ranges_test.rs` (novo).
  - **Status:** Em execução por `QA-Media`.

- [x] **T-045** `[QA-Voice]` — **P0** — **QA/Áudio** — **Testes de regressão para PCM/resampling 44.1↔48kHz, jitter/underrun, Opus PLC, mixer multi-speaker e transições mute/deafen/speaking**
  - **Concluído em 2026-11-24** (layer `qa-voice`, commit `0e4062a`): `tests/audio_engine_regression_test.rs` com 13 testes (10 passaram, **3 falharam expondo bugs reais na voz**: PLC ausente no mixer, soft clip distorcendo mistura de baixo nível e SSRC fantasma após Opus inválido).

- [x] **T-046** `[QA-Msgs]` — **P1** — **QA/UX & Menus** — **Testes de regressão para botões/menus inertes (`T-033`, `T-034`), atalhos anunciados (`T-035`) e layout Friends Hub**
  - **Concluído em 2026-11-24** (layer `qa-msgs`, commit `900a512`): `tests/ux_menus_keybinds_regression_test.rs` com 30 testes (17 passaram, **13 falharam reproduzindo `T-033`/`T-034`/`T-035`**).

- [ ] **T-050** `[QA-Net]` — **P1** — **QA/Privacidade & Auth** — **Testes de redação de logs (`T-042`) e não-persistência prematura de token (`T-040`)**
  - **Arquivos:** `tests/log_redaction_and_auth_safety_test.rs` (novo).
  - **Status:** Em execução por `QA-Net`.

- [x] **T-051** `[QA-Voice]` — **P0** — **QA/Voz DAVE & Reconexão** — **Testes determinísticos de DAVE fail-closed (`T-021`) e renegociação de mídia após `4006`/`4009` (`T-018`)**
  - **Concluído em 2026-11-24** (layer `qa-voice`, commit `5de2e67`): `tests/voice_dave_and_reconnect_regression_test.rs` com 17 testes (4 passaram, **13 falharam expondo bugs reais**: DAVE sem fail-closed/grace limitada, estado parcial em versão inválida, **pânico com external sender malformado**, MLS inválido não fatal e `NewSession 4006/4009` reusando UDP/SSRC/chave antigos).

- [ ] **T-054** `[QA-Voice]` — **P1** — **QA/Voz Vídeo & DM Call** — **Testes de regressão para Captura de Câmera/Tela, Troca de Dispositivos sem bridge no-op (`T-022`/`T-039`) e ciclo completo DM Call (`CALL_CREATE`/`CALL_UPDATE`/`CALL_DELETE`)**
  - **Arquivos:** `tests/voice_video_and_dm_call_regression_test.rs` (novo).
  - **Status:** Em execução por `QA-Voice`.

- [x] **T-052** `[QA-Msgs]` — **P1** — **QA/Histórico & Paginação** — **Testes de paginação infinita `before` (`T-031`), `jump_to_message` `around` (`T-032`) e replies `message_reference` (`T-024`)**
  - **Concluído em 2026-11-24** (layer `qa-msgs`, commit `d2a41fc`): `tests/chat_pagination_reply_regression_test.rs` com 16 testes (2 passaram, **14 falharam reproduzindo `T-024`/`T-031`/`T-032`**).

- [x] **T-053** `[QA-Msgs]` — **P1** — **QA/Guildas & DMs** — **Testes de regressão para ícones de guilda 404/animados (`T-036`), ordenação de DMs por `last_message_id`, migração de pin temporário e Active Now**
  - **Concluído em 2026-11-24** (layer `qa-msgs`, commit `2ca06ab`): `tests/friends_dms_guild_icons_regression_test.rs` com 19 testes (8 passaram, **11 falharam reproduzindo `T-036`/`T-053`**).

- [ ] **T-059** `[QA-Msgs]` — **P2** — **QA/i18n & Busca Avançada** — **Testes de regressão para `i18n` (`pt_br`/`en_us`, `set_locale`, placeholders) e filtros/paginação de Search (`from:`, `in:`, `has:`, `before:`, `after:`)**
  - **Arquivos:** `tests/i18n_and_search_advanced_test.rs` (novo).
  - **Status:** Em execução por `QA-Msgs`.

### Ciclo 2 — Auditoria CDP, funcional e desempenho de 2026-11-24

**Nota de IDs:** durante a auditoria, agentes QA criaram concorrentemente `T-048`..`T-061`. Para evitar colisão no arquivo vivo, as descobertas do Explorer neste ciclo foram renumeradas para **`T-071`..`T-081`**; `T-047` permanece como lacuna reservada no board compartilhado. As medições de shell foram consolidadas de `/tmp/opencode/discord-shell-measurements.md`.

**Metodologia e fontes:** medições somente leitura via CDP em `http://127.0.0.1:9222`, viewport lógico **1018×605**, `devicePixelRatio=2`, cliente oficial Electron em `theme-dark theme-darker images-dark`; screenshots do cliente oficial e de `DiscordRS-SMCodes` (tela 1272×756); código atual da `main` e logs em `/tmp/discord-rs-logs`. Nenhuma mensagem, amizade, configuração ou chamada foi alterada. Upload, drag-and-drop, clipboard de imagem e voz/vídeo remoto já estão cobertos por `T-027`, `T-044` e `T-023`; não foram duplicados aqui.

**Retornos consolidados:**

| Fonte | Evidência principal | Tarefas alimentadas |
|---|---|---|
| `Explorer-UI` | `/tmp/opencode/discord-desktop-measurements.md` e `/tmp/opencode/discord-composer-friends-measurements.md` | `T-073`, `T-079`, `T-012`, `T-034` |
| `Design-Chat-H2` (`hub-task-3025`) | `/tmp/opencode/discord-shell-measurements.md` | `T-071`, `T-072`, `T-078`, `T-079` |
| `Explorer-Deep` | auditoria de código/API/Gateway para threads, permissões, notificações, typing, unread, reconnect e desempenho | `T-076`, `T-077`, `T-080`, `T-081`, `T-062`..`T-067` |
| `Explorer-Live` (`hub-task-3020`) | logs e CDP somente leitura; captura oficial preta/timeout, 14×404 de ícones no RS, foco sem role e fonte efetiva | `T-070`, `T-068`, `T-066`; ícones mantidos em `T-036` |

**Limites de comparação:** idioma, servidor/canal, banner, lista de membros, conteúdo e permissão do composer diferentes não foram tratados como defeitos de layout. Os hexs medidos pertencem ao tema customizado ativo; não foram generalizados para o tema escuro padrão. A captura oficial preta bloqueou diff visual confiável e permanece explicitamente em `T-070`.

- [ ] **T-071** — **P1** — **Design/Guild Rail** — **Alinhar geometria, tema e indicadores do trilho de servidores ao cliente oficial medido**
  - **Arquivos:** `src/components/guild_item.rs:158-161`, `src/components/guild_item.rs:189-205`, `src/components/guild_item.rs:208-245`, `src/components/guild_item.rs:544-576`, `src/app/guild_rail.rs:43-71`, `src/tokens/spacing.rs:230-239`.
  - **Evidência:** medição oficial `/tmp/opencode/discord-shell-measurements.md`: trilho 72px; wrappers 40×40 em `x=16`, passo 48px; separador 32×1/r1; fundo `#0e0b17`, texto forte `#e1e1fe`; pill 8px em `x=-4`, raio `0 4px 4px 0`, alturas 8/20/40; miniaturas de pasta 19×19 com raios `13px 4px 4px`, `4px 13px 4px 4px`, `4px 4px 4px 13px`, `4px 4px 13px`. O ícone **não usa `border-radius`**: é recortado por máscara SVG squircle 40×40 com curvas em `(0,17.4545)`→`(17.4545,0)`; `border-radius:0` não significa quadrado. O RS aproxima com raio 16px, pill 4px em `x=0` e miniaturas 15×15/r10-3.
  - **O que falta/corrigir:** implementar a máscara squircle (ou aproximação explicitamente documentada), tokens de pill/pasta/separador e tema `darker`; corrigir clipping sem achatar a máscara.
  - **Critério de pronto:** screenshots sintéticos lado a lado em repouso, hover, selecionado, unread, menção e pasta fechada/aberta com diferença ≤1px; teste de máscara compara o path/contorno e imagem local cobre todos os estados.

- [ ] **T-072** — **P1** — **Design/Channel Sidebar** — **Corrigir largura, cantos, redimensionamento e painel do usuário**
  - **Arquivos:** `src/app/channel_sidebar.rs:74-119`, `src/app/channel_sidebar.rs:127-163`, `src/app/channel_sidebar.rs:704-715`, `src/app/channel_sidebar.rs:796-902`, `src/app/channel_sidebar.rs:1003-1077`.
  - **Evidência:** medição oficial `/tmp/opencode/discord-shell-measurements.md`: rail 72px + sidebar 272px + handle 4px = **344px**; header 49px/r12 superior esquerdo, nome 16px/600; canal wrapper 34px, link 255×32/r8 com `padding 4px 8px`, ícone 20px e nome 16px/500/24px; cores normal `#8989aa`, selecionado `#e1e1fe`, fundo selecionado `#131316`; painel **328×58** em `x=8`, bottom margin 8px, bloco usuário 153×46, controles 146×32/gap8 e botões 32×32/r8. O RS fixa sidebar em 240px, não tem handle/margem inferior, usa nome 13px bold e subtexto 11px.
  - **O que falta/corrigir:** adotar largura/redimensionamento, cantos, cores, métricas do painel e tipografia; manter voice rows e badges dentro da nova largura.
  - **Critério de pronto:** teste de layout em 1018×605 e 1272×756 confirma 344px no estado padrão, resize persistente, painel 328×58 com margem 8px e links 32px/r8; captura com canal de voz sintético valida participantes, mute/deafen e capacidade.

- [ ] **T-073** — **P1** — **Design/Chat** — **Alinhar Cozy, timestamps de hover e action bar das mensagens**
  - **Arquivos:** `src/chat/message_list.rs:204-278`, `src/chat/message_list.rs:889-902`, `src/chat/message_list.rs:908-965`, `src/chat/message_list.rs:971-1035`.
  - **Evidência:** CDP oficial `/tmp/opencode/discord-desktop-measurements.md`: início de grupo com `margin-top:17px` e `min-height:44px`; continuação `min-height:22px`; padding cozy `2px 72px 2px 24px`; avatar 40×40 em `x=16/y=2`; username 16px/500/22px, timestamp 12px/500/22px com margem 4px e corpo 16px/400/22px; hover `#181422` com raio 4px apenas à direita. Action bar externa 229×34; interna **183×34/r8**, padding 2px, borda `#1c1a27`, fundo `#1a1725`, seis botões 28×28/r6/ícones 20px e separador 9px. O RS usa gutter 48px + `pl16px`, action bar de três botões, fundo `bg_chat`, raio 6px e não oferece editar/fixar no hover.
  - **O que falta/corrigir:** ajustar grid/padding/altura, timestamp compacto e barra completa de ações contextuais; respeitar mensagem própria e permissões.
  - **Critério de pronto:** fixture sintética com grupo, mensagem isolada, bot, editada, reply, embed e reação gera captura ≤1px de diferença; hover mostra todos os ações permitidos e timestamp sem deslocar conteúdo.

- [ ] **T-074** — **P1** — **Design/Perfil** — **Reduzir modal de perfil e corrigir avatar/banner/tabs ao oficial**
  - **Arquivos:** `src/app/user_profile_popout.rs:340-351`, `src/app/user_profile_popout.rs:434-460`, `src/app/user_profile_popout.rs:492-497`, `src/app/user_profile_popout.rs:936-965`, `src/app/user_profile_popout.rs:991-1017`, `src/app/user_profile_popout.rs:1049-1054`.
  - **Evidência:** CDP oficial: popout 300×348/r8, banner 105px e avatar 80px (estes já batem); modal **600×557/r8**, banner 210px, avatar **120px**, tabs em `y=397` com 25px de altura. O RS abre modal **1184×704/r12** e usa avatar 80px, além de largura fixa que não cabe em viewports menores.
  - **O que falta/corrigir:** modal 600px responsivo, avatar 120px, offsets/tabs medidos, remover borda/cor sem origem e limitar altura à viewport com scroll interno.
  - **Critério de pronto:** capturas de popout e modal em 1018×605 e 800px de largura confirmam dimensões, sobreposição do avatar, banner, tabs e fechamento por Esc/clique fora; decoração e efeito continuam ancorados ao avatar/banner.

- [ ] **T-075** — **P1** — **Design/Settings** — **Alinhar shell responsivo e navegação de User Settings**
  - **Arquivos:** `src/app/user_settings/mod.rs:140-155`, `src/app/user_settings/mod.rs:169-288`, `src/app/user_settings/mod.rs:325-370`, `src/app/user_settings/mod.rs:441-463`.
  - **Evidência:** CDP oficial em 1018×605: sidebar 253px, conteúdo 766px, header 49px iniciando em `y=33`, item de navegação 220×36/r8 com padding 8px e ativo `rgba(151,151,159,0.16)`. O RS usa sidebar 240px, conteúdo fixo 700px, topo 60px e itens 192×32.
  - **O que falta/corrigir:** shell fluido `253px + flex`, header/top offsets e itens de 36px; remover largura fixa que estoura em janelas pequenas.
  - **Critério de pronto:** screenshots de todas as abas em 1018×605, 1272×756 e 800×600 sem overflow; navegação, busca e fechamento por Esc funcionam; medidas ativas/inativas batem com o CDP.

- [ ] **T-076** — **P1** — **Chat/Threads/Forum** — **Adicionar tipos de canal Forum, Threads, Stage e Directory sem descartá-los no parser**
  - **Arquivos:** `src/domain/channel.rs:6-47`, `src/domain/channel.rs:148-191`, `src/app/state/models.rs:118-143`, `src/protocol/discord_api.rs:2205-2208`, `src/app/state/gateway_events.rs:290-307`.
  - **Evidência:** `ChannelType::from_discord_code` só aceita 0..5; `parse_channel_from_json` retorna `None` para tipos 10–15. `ChannelKind` só tem Text/Voice/Announcement. Forum, threads, Stage Voice e Directory desaparecem da sidebar e não podem ser abertos.
  - **O que falta/corrigir:** modelar tipos 10–15, `parent_id`, `thread_metadata`, `default_tag_ids`, `available_tags` e flags de forum; criar ícones, ordenação e view de forum em grade/lista.
  - **Critério de pronto:** fixture da API com cada tipo 10–15 é preservada e renderizada; fórum mostra tags, contadores, ordenação e criação de post; Stage Voice entra no fluxo de voz sem virar canal de texto.

- [ ] **T-077** — **P1** — **Chat/Threads** — **Implementar ciclo de vida de threads e eventos do Gateway**
  - **Arquivos:** `src/protocol/transport.rs:125-146`, `src/protocol/discord_api.rs:2916-3062`, `src/app/state/gateway_events.rs:9-754`, `src/chat/chat_view.rs:1795-1808`, `src/app/header_bar.rs:309-487`.
  - **Evidência:** não existem rotas de list/create/join/archive/lock/delete thread nem eventos `THREAD_CREATE/UPDATE/DELETE`, `THREAD_MEMBER_UPDATE` ou `THREAD_LIST_SYNC`; `ChannelCreate` é declarado, mas não tratado no `AppState`. O composer não oferece “Criar tópico”.
  - **O que falta/corrigir:** endpoints oficiais, inscrição `GUILD_MEMBERS`/threads quando necessário, lista de threads sob o canal pai, header de thread ativa, criação/entrada/saída/arquivamento e reconciliação em tempo real.
  - **Critério de pronto:** testes sintéticos cobrem create/update/delete/list-sync e perda de mensagem; UI cria thread, mostra membros/estado e navega entre pai/thread sem duplicar mensagens.

- [ ] **T-080** — **P1** — **Permissões/UX** — **Calcular permissões reais e ocultar/bloquear ações sem autorização**
  - **Arquivos:** `src/domain/guild.rs:5-23`, `src/domain/channel.rs:148-191`, `src/app/state/models.rs:147-170`, `src/chat/editor.rs:419`, `src/chat/chat_view.rs:1795-1808`, `src/app/channel_sidebar.rs:546-793`.
  - **Evidência:** só `Role.permissions` é guardado; não há `permissions` do membro atual nem `permission_overwrites` no canal. `ChatComposer` nasce habilitado e a sidebar renderiza todos os canais. No canal oficial inspecionado, o composer exibia “You do not have permission to send messages in this channel”; o RS não tem como reproduzir esse estado.
  - **O que falta/corrigir:** resolver `@everyone` + cargos + overwrites de canal/categoria, com allow/deny; esconder canais sem `VIEW_CHANNEL`, travar composer/ações sem `SEND_MESSAGES`, `ADD_REACTIONS`, `ATTACH_FILES`, `CONNECT`/`SPEAK`.
  - **Critério de pronto:** testes de bitfield cobrem administrador, dono, negação explícita, timeout e overwrite de categoria; UI sintética comprova canal oculto, composer desabilitado e ações removidas antes de qualquer request.

- [ ] **T-081** — **P1** — **Notificações** — **Implementar notificações desktop, sons e badge de unread reais**
  - **Arquivos:** `Cargo.toml:6-34`, `src/app/user_settings/notifications.rs:9-174`, `src/app/state/gateway_events.rs:109-224`, `src/app/state/models.rs:57-64`.
  - **Evidência:** Settings só alterna campos locais; não há backend de notificação, som ou badge. `MessageCreate` atualiza histórico/ordenação, mas não emite notificação. O projeto não depende de `notify-rust`/serviço D-Bus de notificações.
  - **O que falta/corrigir:** integração desktop oficial (freedesktop/D-Bus), som real configurável, badge de tarefa/unread, respeito a DND e escopo todas/@menções/nada; agrupar por canal e não notificar mensagem própria.
  - **Critério de pronto:** testes com backend injetado validam menção, DM, canal mutado, DND e mensagem própria; teste manual em Linux exibe notificação/som/badge e remove o badge ao ler.

- [ ] **T-062** — **P1** — **Chat/Typing** — **Enviar `TYPING_START` e expirar indicadores sem timer ativo**
  - **Arquivos:** `src/app/state/messages.rs:405-451`, `src/protocol/rate_limit/coalescing.rs:35-91`, `src/protocol/discord_api.rs:1887-1897`, `src/chat/chat_view.rs:615-621`.
  - **Evidência:** o recebimento é registrado, mas não há `POST /channels/{id}/typing`; `should_send_typing` existe e só é usado em testes. O TTL de 9s é filtrado apenas quando alguém lê o vetor; sem timer, o indicador pode permanecer desenhado até outro render.
  - **O que falta/corregir:** disparar typing no editor com coalescing oficial, limpar no envio e agendar expiração que chame `notify`; resolver nome por membro/amigo/DM sem exibir ID bruto.
  - **Critério de pronto:** teste determinístico com relógio injetado comprova um request por janela, parada após envio e remoção visual em 9s; Gateway sintético cobre vários usuários e canal trocado.

- [ ] **T-063** — **P1** — **Chat/Unread** — **Implementar read state, badges e barra de não lidas/jump to present**
  - **Arquivos:** `src/app/state/models.rs:147-170`, `src/app/state/gateway_events.rs:72-83`, `src/app/state/gateway_events.rs:109-224`, `src/app/state/guilds_channels.rs:281-323`, `src/protocol/discord_api.rs:1268-1275`, `src/chat/chat_view.rs:602-612`.
  - **Evidência:** canais nascem com `unread=false` e `mention_count=0`; `MessageCreate` não incrementa esses campos; não há ACK de leitura nem modelo de `last_read_message_id`. O oficial mostra badges, divisor “NEW” e barra de não lidas; o RS não renderiza nenhum desses estados.
  - **O que falta/corregir:** manter `read_state_version`, `last_message_id`, `last_read_message_id` e contagem de menções; ACK ao visualizar; divisor/new badge e botão “Jump to Present” quando afastado do fim.
  - **Critério de pronto:** fixture com 100 mensagens, menções e leitura parcial comprova badges, divisor, ACK uma vez e salto preservando o destino; troca de canal limpa o estado correto.

- [ ] **T-064** — **P1** — **Rede/Reconnect** — **Preservar o shell autenticado e mostrar banner de reconexão**
  - **Arquivos:** `src/protocol/discord_api.rs:1202-1207`, `src/protocol/discord_api.rs:1387-1419`, `src/app/state/gateway_events.rs:275-287`, `src/app/root_view.rs:554-580`.
  - **Evidência:** cada fechamento de socket emite `TransportEvent::Disconnected`, e `RootView` troca imediatamente toda a interface pela tela de login quando `is_connected_to_gateway=false`. Não existe estado `Reconnecting`; o loop reconecta em segundo plano, causando perda visual de contexto e cache.
  - **O que falta/corrigir:** estados Connecting/Connected/Reconnecting/Offline, manter shell/cache durante retry, banner discreto com tentativa/backoff e recuperação silenciosa após READY/RESUME.
  - **Critério de pronto:** teste de socket sintético cobre Op7, Op9, EOF e falha de DNS; UI permanece na tela atual, mostra banner e volta a conectar sem solicitar token novamente.

- [ ] **T-065** — **P1** — **Performance/UI** — **Remover clones e enumeração de hardware do hot path de render**
  - **Arquivos:** `src/app/guild_rail.rs:28-41`, `src/app/voice_call_panel.rs:122-146`, `src/chat/chat_view.rs:351-391`, `src/chat/chat_view.rs:531-600`, `src/app/friends_hub.rs:898-919`.
  - **Evidência:** `GuildRail::render` clona a árvore inteira de guildas/pastas a cada render (o log inicial carregou 200 guildas); `VoiceCallPanel` cria `MediaDeviceManager` e enumera entrada/saída em todo frame; `ChatView` recria `Arc<Vec<ChatListItem>>`; Friends renderiza todas as linhas sem virtualização.
  - **O que falta/corregir:** borrow/snapshots `Arc`, cache de dispositivos atualizado por evento, virtualização de listas grandes e memoização de métricas de layout.
  - **Critério de pronto:** benchmark com 200 guildas/10k mensagens/1k amigos mantém montagem <8,33ms p95 e não cresce memória linearmente a cada frame; enumeração de dispositivos não ocorre no thread de render.

- [ ] **T-066** — **P1** — **Design/Tipografia/Tema** — **Alinhar fonte e tema ao cliente oficial sem usar ativo não licenciado**
  - **Arquivos:** `src/main.rs:30-33`, `src/tokens/kit_theme.rs:285-310`, `src/tokens/typography.rs`, `src/app/state/settings.rs:82-110`.
  - **Evidência:** CDP oficial usa `gg sans`, 16px/22px e classe `theme-darker`; o log do RS informa que `.SystemUIFont` resolve para **Adwaita Sans**. `main` instala apenas o tema escuro uma vez e `AccountThemeUpdated` não reaplica paleta.
  - **O que falta/corregir:** usar `gg sans` somente se houver licença; caso contrário, adotar fallback aberto com métricas documentadas e cessar alegação de pixel-parity tipográfica; aplicar Dark/Light/Darker/Midnight e custom colors da conta.
  - **Critério de pronto:** teste de métricas compara largura/altura de glifos e line-height; screenshots registram a fonte efetiva; troca de tema reaplica cores sem reiniciar e não distribui fonte proprietária sem autorização.

- [ ] **T-067** — **P1** — **Performance/Telemetria** — **Instrumentar FPS, frame time, RSS e long tasks de verdade**
  - **Arquivos:** `src/telemetry/service.rs:82-93`, `src/telemetry/types.rs:5-17`, `src/telemetry/reporter.rs:192`, `examples/ui_shot.rs`.
  - **Evidência:** `track_render` existe, mas não tem nenhum call site; só há durations manuais de interação. Não são medidos FPS, p95/p99 de frame, RSS/heap ou tarefas longas, portanto as metas de 8,33/16,66ms não são comprovadas.
  - **O que falta/corregir:** instrumentar render/frame e memória com baixo overhead, exportar percentis e adicionar modo de stress reproduzível.
  - **Critério de pronto:** relatório gerado por cenário sintético contém FPS, p50/p95/p99, RSS e pico; teste de regressão falha se o orçamento for excedido em máquina controlada.

- [ ] **T-068** — **P2** — **Acessibilidade/UX** — **Expor papéis, nomes e foco acessível na árvore GPUI**
  - **Arquivos:** `src/components/avatar.rs:240-283`, `src/chat/message_list.rs:889-902`, `src/app/header_bar.rs:317-487`, `src/app/user_settings/mod.rs:441-463`.
  - **Evidência:** `/tmp/discord-rs-logs/smcodes.log:44` registra elemento focado sem nó de acessibilidade (“has an id but no role”). O CDP oficial publica `navigation`, `list`, `article`, `button`, `heading` e labels; vários componentes RS usam `.id(...)` sem `.role(...)`/label.
  - **O que falta/corregir:** roles e labels em avatar, mensagem, canais, botões e tabs; ordem de foco, foco visível e anúncio de estados; garantir que ícones decorativos não sejam anunciados.
  - **Critério de pronto:** snapshot de acessibilidade sintético contém mensagens como `article`, ações como `button` e nomes significativos; navegação por teclado cobre chat, sidebar, perfil e Settings sem armadilha de foco.

- [ ] **T-069** — **P1** — **Chat/DM/Regressão** — **Eliminar de vez os canais legados de demonstração e restaurar o teste de higiene de DMs**
  - **Arquivos:** `src/app/state/gateway_events.rs:387-435`, `tests/dm_messaging_test.rs:170-207`, `src/app/state/seed.rs`.
  - **Evidência:** a suíte completa do Ciclo 2 executou `dm_messaging_test` com **5 passaram e 1 falhou**: `test_no_hardcoded_ferris_dm_and_clean_state` afirma que `dm-ferris` sobreviveu a `DmChannelsUpdated`. O código ainda carrega uma blocklist explícita de `dm-ferris`, `dm-group-gpui`, `dm-ana` e `user-ferris` em vez de garantir que apenas DMs temporários legítimos sejam preservados.
  - **O que falta/corrigir:** remover IDs/nomes de demonstração do modelo e da persistência; preservar somente `dm-*` criados após uma ação real do usuário e ainda não resolvidos; investigar por que a blocklist atual não elimina o item no teste.
  - **Critério de pronto:** `cargo test --test dm_messaging_test` passa 6/6; fixture com DM legado, DM temporário real e DM do Gateway comprova remoção/preservação corretas; nenhum ID de demonstração permanece em produção.

- [ ] **T-070** — **P1** — **QA/Captura Visual** — **Tornar o pipeline de screenshots confiável e comparável**
  - **Arquivos:** `examples/ui_shot.rs:1-20`, `scripts/ui-shot.sh`, `scripts/ui-compare.sh`, `tests/fixtures/ui_shot/`.
  - **Evidência:** o relatório `Explorer-Live` registrou captura oficial totalmente preta e timeout de `Page.captureScreenshot` apesar do DOM visível; `/tmp/discord-ciclo2-oficial-channel.png` e `/tmp/discord-ciclo2-rs-channel.png` estavam em servidores/canais, idiomas, banners, membros e estados de composer diferentes. O harness atual usa fixture offline e `import -window`, sem validar imagem preta, viewport, DPR ou equivalência de estado.
  - **O que falta/corregir:** validar captura não preta/não vazia, registrar viewport/DPR/título/rota sem dados pessoais, exigir mesma fixture/estado nas duas pontas e ter fallback de captura com timeout/log de causa.
  - **Critério de pronto:** script sintético gera par comparável com checksum/dimensões iguais, falha explicitamente em imagem preta ou timeout e nunca salva IDs/nomes/conteúdo real no repositório.

- [ ] **T-078** — **P1** — **Design/Header/Search** — **Alinhar Header Bar, botões e campo de busca às medidas oficiais**
  - **Arquivos:** `src/app/header_bar.rs:133-158`, `src/app/header_bar.rs:351-408`, `src/app/header_bar.rs:422-485`, `src/app/header_bar.rs:557-585`.
  - **Evidência:** `/tmp/opencode/discord-shell-measurements.md`: header 49px, padding `8px 8px 8px 16px`, conteúdo/toolbar 32px, gap 8px; botões 32×32 com SVG 20×20, cor `#9999b9` e hover `#e1e1fe`; busca **244×32/r8**, editor 218×22, fonte 14px/500/22px, placeholder `#9493b4` e gradiente de fundo real. O RS usa botões 28×28 com ícones 16px e busca fixa 144px.
  - **O que falta/corrigir:** ajustar header/toolbar/botões/busca, gradiente e transição de largura; preservar foco, clear e popover de filtros.
  - **Critério de pronto:** captura sintética em 1018×605 e 1272×756 confirma header 49px, botões 32px e busca 244×32/r8; teste de foco/clear/filtros passa sem deslocar o toolbar.

- [ ] **T-079** — **P2** — **Design/Member Sidebar** — **Corrigir largura e hierarquia visual da lista de membros**
  - **Arquivos:** `src/app/member_sidebar.rs:145-165`, `src/app/member_sidebar.rs:230-245`, `src/app/member_sidebar.rs:275-305`.
  - **Evidência:** `/tmp/opencode/discord-desktop-measurements.md` mediu Member Sidebar com **264px** de conteúdo (265px incluindo borda), grupo útil 256×42/padding `20px 4px 4px 16px`, header 18px/14px/500, linha 248×42 sem subtexto ou 248×44 com status, avatar 32px/margem 12px, status 10px em `left/top:22px` e nome 16px/500/20px. O RS fixa **240px** (`src/app/member_sidebar.rs:156`), header 40px/texto 12px semibold (`:203-215`), linha sempre 44px/gap10 (`:232-245`) e nome 14px semibold (`:275-288`).
  - **O que falta/corrigir:** adotar 264px e as métricas medidas de grupo/header/linha/avatar/status; medir no CDP ícone de cargo, offline e hover antes de ajustar cores/subtexto.
  - **Critério de pronto:** captura sintética com cargos, bot, atividade e offline confirma largura 264px, header 18px e linhas 42/44px sem overflow; teste de toggle da sidebar preserva a largura do chat.

### Ciclo 3 — Navegação, autocomplete, convites, Group DMs, markdown, menus, mídia animada e responsividade de 2026-11-24

**Metodologia e limites:** auditoria somente leitura da `main` e do Discord Desktop já aberto. O CDP foi acessado por `list_pages` + `select_page` na aba existente (Friends); **nenhum `new_page`, clique, digitação, navegação ou alteração de conta**. As evidências oficiais vêm de CSS/DOM lido por `evaluate_script`, do bundle `/tmp/opencode/discord-web-entry.js` e do locale `/tmp/opencode/discord-official-en-US.js`. Viewport observado: **1017×605**, `devicePixelRatio=1.4375`, tema customizado `theme-dark theme-darker`. Quick Switcher e autocomplete não foram abertos por interação: sua existência/geometria foi comprovada por CSS e bundle, não por screenshot. Nenhum ID, nome, avatar ou conteúdo pessoal foi registrado.

- [ ] **T-082** — **P1** — **UX/Navegação** — **Implementar Quick Switcher global (Ctrl/Cmd+K) para DMs, canais e servidores**
  - **Arquivos:** `src/chat/chat_view.rs:1812-1868`, `src/app/user_settings/keybinds.rs:58-102`, `src/app/state/models.rs:57-67`, `src/app/root_view.rs:691-783`, `src/app/state/guilds_channels.rs`, `src/app/state/friends_dms.rs`.
  - **Evidência:** o handler de teclado auditado no RS só reconhece `Ctrl+F`, Escape e ArrowUp; busca por `quick.?switch|ctrl.?k|command.?palette` não encontra implementação. No oficial, `/tmp/opencode/discord-web-entry.js:35` exibe o atalho `mod+k`; `:232` contém `QuickSwitcherStore` com `channelHistory` e resultados de navegação/convites. O CSS CDP observado `.quickswitcher_a1DYes` usa `display:flex`, coluna, `padding:12px 20px 0`, raio 8px e cor `--text-default`.
  - **O que falta/corrigir:** estado global de overlay, índice fuzzy de guildas/canais/DMs permitidos, histórico recente, teclado (setas/Tab/Enter/Esc), foco acessível e navegação sem fechar o shell autenticado.
  - **Critério de pronto:** `Ctrl+K` e `Cmd+K` abrem o switcher em qualquer tela autenticada; fixtures sintéticas com nomes parecidos, canal sem permissão e resultado vazio validam ranking, navegação e fechamento; nenhum ID real aparece em log ou snapshot.

- [ ] **T-083** — **P1** — **Chat/Autocomplete** — **Adicionar popup de @menção, #canal e :emoji: com seleção por teclado**
  - **Arquivos:** `src/chat/editor.rs:202-279`, `src/chat/editor.rs:409-438`, `src/chat/editor.rs:537-642`, `src/chat/chat_view.rs:224-279`, `src/chat/chat_view.rs:1812-1868`, `src/app/state/messages.rs:269-280`.
  - **Evidência:** `InputEvent::Change` do RS apenas copia o texto para `typed_text`; Enter envia e não existe estado de sugestões, âncora, seleção ou popup. O CDP oficial mediu `.autocomplete_rg6La1` com `min-width:430px`, `max-width:800px`, raio 5px, sombra `--shadow-high`; `.autocompleteAttached_rg6La1` fica `bottom:calc(100% + 8px)` com insets 8/16px. Linhas usam 14px/500/16px, padding `4px 8px`, ícone com margem 8px e secundário com margem 16px; o grid de stickers usa colunas 116px e gap 8px.
  - **O que falta/corrigir:** detectar token sob o cursor, filtrar membros/canais/emojis reais, ancorar o popup ao composer, substituir apenas o token, suportar setas/Tab/Enter/Esc e não roubar Enter quando não houver seleção.
  - **Critério de pronto:** testes determinísticos cobrem `@`, `#`, `:`, consulta vazia, acentos, emoji customizado animado, seleção por setas/Tab/Enter e cancelamento; a mensagem enviada contém `<@id>`, `<#id>` ou `<a:name:id>` corretos, sem nome fabricado.

- [ ] **T-084** — **P1** — **Chat/Convites** — **Resolver `discord.gg/...`, renderizar embed e abrir modal real de entrar**
  - **Arquivos:** `src/chat/markdown.rs:85-91`, `src/chat/markdown.rs:451-475`, `src/chat/markdown.rs:838-844`, `src/chat/message_list.rs:762-859`, `src/protocol/discord_api.rs:292-306`, `src/protocol/transport.rs:238-297`, `src/app/state/models.rs:57-67`, `src/app/invite_modal.rs` (novo).
  - **Evidência:** o RS trata a URL como `InlineSpan::Link` sem `on_click`; não há rota `/invites/{code}` nem estado de convite. O bundle oficial define ``INVITE:e=>`/invites/${e}` `` em `/tmp/opencode/discord-web-entry.js:23` e resolve links de markdown para `INVITE_MODAL_OPEN`. O CSS CDP observou card `guildInviteContainer` com 300px/r16, splash 64px, `inviteEmbed` r12, título com margem inferior 12px e linha de membro com gap 8px.
  - **O que falta/corrigir:** parser de domínio/código, GET do convite, embed com splash/ícone/membros/estado, clique seguro, modal Entrar com confirmação da API e tratamento de expirado/inválido/banido/já-membro.
  - **Critério de pronto:** fixture sintética valida código, domínio inválido, 404/429, expiração e já-membro; clique abre o modal, Enter só navega após resposta da API e nenhum sucesso é mostrado antes da confirmação.

- [ ] **T-085** — **P1** — **Servidores/Criação** — **Tornar “Criar o Meu Próprio” uma criação real de servidor**
  - **Arquivos:** `src/app/root_view.rs:1315-1374`, `src/app/state/models.rs:57-67`, `src/protocol/discord_api.rs`, `src/protocol/transport.rs:238-297`, `src/protocol/client_manager.rs:534-550`, `src/app/state/guilds_channels.rs`.
  - **Evidência:** o botão `create-server-action` em `root_view.rs:1357-1360` não tem `on_click`; não existe `POST /guilds` nem atualização do trilho após criação. O oficial oferece o fluxo por `mod+shift+n` (`/tmp/opencode/discord-web-entry.js:81`) e possui mensagem “server created successfully” no locale (`discord-official-en-US.js:1`).
  - **O que falta/corrigir:** formulário de nome/ícone opcional, chamada oficial de criação, estado pendente/erro, inserção da guilda retornada, seleção do canal inicial e atualização do cache/rail somente após sucesso.
  - **Critério de pronto:** teste HTTP sintético valida payload e resposta; UI mantém o modal em erro sem guilda fictícia, mostra sucesso após 2xx e exibe a guilda/canal reais; cancelamento não cria nada.

- [ ] **T-086** — **P1** — **Chat/Group DMs** — **Substituir Group DMs locais por ciclo de vida real via API e Gateway**
  - **Arquivos:** `src/app/state/friends_dms.rs:373-427`, `src/app/root_view.rs:1432-1760`, `src/protocol/discord_api.rs:292-306`, `src/protocol/discord_api.rs:2205-2277`, `src/domain/channel.rs:148-191`, `src/app/state/models.rs:147-170`, `src/app/state/gateway_events.rs:387-535`.
  - **Evidência:** `create_group_dm` cria ID `dm-group-<timestamp>` e `add_recipient_to_group` só altera vetores locais; não há rename, remoção, saída nem ícone real. O oficial usa `POST /users/@me/channels` com `recipients`, `PUT/DELETE /channels/{id}/recipients/{user}` e `CHANNEL_ICON` (`/tmp/opencode/discord-web-entry.js:23`); o locale oficial inclui “Group changes saved”, “user added to the group” e “You have left the group”.
  - **O que falta/corrigir:** criar com lista de destinatários, resolver ID real, adicionar/remover, renomear, trocar/remover ícone, sair e reconciliar `CHANNEL_CREATE/UPDATE/DELETE`; remover confirmação otimista fabricada.
  - **Critério de pronto:** testes sintéticos cobrem criação, adição/remoção, rename, ícone, saída e falha com rollback; após reinício o Group DM permanece com ID da API e membros/ícone reais, sem `dm-group-*` temporário persistido.

- [ ] **T-087** — **P1** — **Chat/Markdown** — **Corrigir headings, listas, masked links, blockquotes e resolução de menções**
  - **Arquivos:** `src/chat/markdown.rs:49-55`, `src/chat/markdown.rs:124-243`, `src/chat/markdown.rs:376-418`, `src/chat/markdown.rs:451-475`, `src/chat/markdown.rs:662-686`, `src/chat/markdown.rs:838-844`, `tests/attachments_embeds_markdown_test.rs`.
  - **Evidência:** o AST já possui `Heading`, `Subtext` e `BlockQuote`, mas não possui lista ordenada/bullet; links masked são apenas `div` sem handler; menções Snowflake exibem nomes inventados `user-####`, `canal-####` e `cargo-####`. O oficial renderiza `<ol start>`/`<ul>` (`/tmp/opencode/discord-web-entry.js:232`) e tem confirmação de domínio/protocolo para masked links (`:257`). O CSS CDP mede blockquote com divisor 4px/r4, padding `12px 8px`, margem 4px e `max-width:90%`.
  - **O que falta/corrigir:** nós de lista com início/indentação, links clicáveis e seguros, validação visual de headings/subtext/blockquote contra o oficial, resolução de IDs por membro/canal/cargo real e fallback neutro quando a entidade não existe.
  - **Critério de pronto:** fixture cobre `#`, `##`, `###`, `-#`, `1.`/`3.`, `-`/`*`, `[texto](url)`, `>>>` multilinha e menções; snapshot valida semântica/medidas e nenhum nome sintético é exibido como dado da conta.

- [ ] **T-088** — **P1** — **UX/Menus de Contexto** — **Completar menus de servidor, pasta, categoria e canal de voz com ações reais**
  - **Arquivos:** `src/components/guild_item.rs:632-687`, `src/app/guild_rail.rs:43-71`, `src/app/channel_sidebar.rs:522-539`, `src/app/channel_sidebar.rs:603-640`, `src/app/channel_sidebar.rs:772-784`, `src/app/channel_sidebar.rs:796-859`, `src/app/state/guilds_channels.rs`, `src/protocol/discord_api.rs`.
  - **Evidência:** o menu de canal do RS cria “Marcar como lido”, “Silenciar canal” e “Copiar link do canal” sem `on_click`; servidor, pasta e categoria não têm menu de contexto próprio e a linha de voz só responde a clique. O locale oficial contém “Mark Server As Read”, “Mute Server”, “Mute this category”, “Mute this channel”, “Copy Server ID”, “Copy Channel ID”, “Copy Category ID” e “Developer Mode” (`/tmp/opencode/discord-official-en-US.js:1`).
  - **O que falta/corrigir:** matriz de menus por tipo/permissão, mute com duração, mark-read, copy ID somente em Developer Mode, configurações/editais reais e fechamento/foco acessível; estender `T-034` sem duplicar o menu de mensagem já existente.
  - **Critério de pronto:** testes de UI/API cobrem cada item visível, permissão negada, durações de mute e clipboard sintético; nenhum item do menu é no-op e IDs reais não vão para logs.

- [ ] **T-089** — **P1** — **Mídia/Animação** — **Reproduzir GIF/APNG, avatares `a_`, stickers Lottie e emojis animados sem fallback silencioso**
  - **Arquivos:** `Cargo.toml:32`, `src/domain/user.rs:447-455`, `src/domain/message.rs:43-50`, `src/domain/message.rs:157-180`, `src/protocol/discord_api.rs:2193-2198`, `src/protocol/discord_api.rs:2258-2277`, `src/protocol/media_cache.rs:128-225`, `src/components/avatar.rs:205-268`, `src/chat/message_list.rs:605-760`, `src/chat/markdown.rs:854-876`.
  - **Evidência:** `User::avatar_url`, `Author::avatar_url` e o parser do usuário atual sempre usam `.png`, mesmo para hash `a_`; `StickerFormatType::Lottie` cai no fallback `.webp`; anexos de imagem não usam `att.url`; `image` está compilado só com JPEG/PNG e não há runtime Lottie no projeto. O bundle oficial mapeia APNG→`png`, Lottie→`json` e GIF→`gif` (`/tmp/opencode/discord-web-entry.js:236`) e o CSS CDP contém `.lottieContainer`/`.lottie` com `overflow:hidden` e dimensão integral.
  - **O que falta/corrigir:** normalizar URLs animadas, decoder/cache de múltiplos quadros, renderer Lottie, integração de attachments (sem duplicar o upload de `T-027`), reduced motion e fallback estático explícito em falha.
  - **Critério de pronto:** fixtures GIF/APNG de dois quadros, avatar `a_`, emoji `<a:...>` e sticker Lottie comprovam reprodução/cache; reduced motion pausa ou reduz a animação; falha de CDN mostra fallback estático, nunca imagem quebrada ou animação fabricada.

- [ ] **T-090** — **P1** — **UX/Responsividade** — **Definir largura mínima, colapso de sidebars e overflow seguro ao redimensionar**
  - **Arquivos:** `src/main.rs:35-62`, `src/app/root_view.rs:691-768`, `src/app/channel_sidebar.rs:74-119`, `src/app/member_sidebar.rs:155-165`, `src/app/friends_hub.rs:66-75`, `src/app/friends_hub.rs:1511-1518`, `src/app/user_settings/mod.rs:140-155`, `src/app/user_settings/mod.rs:340`, `src/app/user_profile_popout.rs:949`.
  - **Evidência:** a janela nasce com clamp 940–1280px, mas `WindowOptions` não define `min_width`; o shell sempre monta rail + sidebar + member sidebar e vários modais têm largura fixa. No CDP oficial a 1017×605, shell=344px e página=673px; ao emular larguras menores, a sidebar permaneceu 344px e a página encolheu até 0px, sem overflow documental. O CSS oficial esconde `inviteToolbar` em `max-width:940px` e torna membros full-width em `max-width:485px`.
  - **O que falta/corrigir:** breakpoints explícitos, min width ou drawers, auto-ocultar membros/Active Now, clamp de modais e preservação de estado ao redimensionar; consolidar `T-072`/`T-074`/`T-075`/`T-079` em um comportamento de janela único.
  - **Critério de pronto:** matriz 1017×605, 940×600, 800×600, 600×600 e 485×600 sem página de largura zero, overflow horizontal ou controles inacessíveis; resize persistente e toggles restauram sidebars sem perder canal/seleção.
