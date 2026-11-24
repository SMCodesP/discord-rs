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
- [ ] **T-001** `[Voice-Audio]` — **Diagnosticar e corrigir voz bugada na call (áudio picotado/robótico/mudo/dessincronizado)**
  - **Escopo**: Auditar `src/media/audio.rs`, `src/media/capture.rs`, `src/media/codec.rs`, `src/media/mixer.rs`, `src/media/resample.rs`, `src/media/devices.rs`.
  - **Suspeitas iniciais**: Taxa de amostragem do dispositivo `cpal` != 48kHz (resampling incorreto ou faltando), frame size Opus (960 samples @ 48kHz = 20ms) desalinhado, canais mono vs stereo, jitter buffer ausente/estourando, bloqueio na thread de áudio, silêncio/DTX ou sequência RTP/timestamp.
  - **Critério de pronto**: Causa raiz documentada, correção implementada, testes cobrindo 44.1kHz/48kHz mono/stereo e framing 20ms passando.

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

- [ ] **T-013** `[Design-Friends]` — **Paridade visual & funcional: Friends Hub, Active Now, DMs Fixadas, Presença**
  - **Escopo**: `src/app/friends_hub.rs`, `src/app/state/friends_dms.rs`, `src/protocol/pinned_dms.rs`.
  - **Foco**: Abas Online/Todos/Pendentes/Bloqueados/Adicionar Amigo, cartões "Ativo Agora" com imagens reais de atividade e barra de progresso Spotify, presença (online/idle/dnd/offline/custom status).

- [ ] **T-014** `[Design-Settings]` — **Paridade visual & funcional: User Settings (todas as abas) + Zero Mocks**
  - **Escopo**: `src/app/user_settings/*`, `src/app/state/settings.rs`, `src/app/state/seed.rs`.
  - **Foco**: Garantir 0 dados fictícios em produção, abas fiéis ao Discord, seleção real de dispositivos de áudio/vídeo em Voz e Vídeo, troca de idioma e tema funcionando.

- [ ] **T-015** `[QA-Visual]` — **Auditoria Visual Automatizada & Comparação com Discord Oficial**
  - **Escopo**: `examples/ui_shot.rs`, `tests/tokens_spec_test.rs`, `tests/kit_theme_test.rs`, `tests/guild_icon_radius_test.rs`, `tests/telemetry_service_test.rs`.
  - **Foco**: Comparar screenshots do `discord-rs` com `Discord-Oficial` via `mao portal screenshot`, catalogar divergências exatas (px, hex, layout) na seção final.

### P2 — Rede, Gateway, Cache, Auth e Estabilidade
- [ ] **T-016** `[Net-Gateway]` — **Auditoria de Gateway, REST, Rate Limiter, Reconexão, Cache e Protobuf**
  - **Escopo**: `src/protocol/discord_api.rs`, `src/protocol/client_manager.rs`, `src/protocol/rate_limit/*`, `src/protocol/cache/*`, `src/protocol/user_settings_proto.rs`, `src/app/state/gateway_events.rs`, `src/app/state/connection.rs`.
  - **Foco**: Reconexão robusta (RESUME vs IDENTIFY), tratamento de 429/rate-limit buckets, evitar race conditions, zero vazamentos de memória, logs estruturados sem segredos.

- [ ] **T-017** `[QA-Net]` — **Suíte dedicada de testes de Rede, Cache, Rate Limit, Auth e E2E**
  - **Escopo**: `tests/protocol_tests.rs`, `tests/protocol_bridge_test.rs`, `tests/cache_and_ratelimit_test.rs`, `tests/login_authentication_test.rs`, `tests/e2e_full_flow.rs`, `tests/purger_hygiene_test.rs`, `tests/user_settings_proto_test.rs`.

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
  - **Evidência:** busca de DM, Nitro, Loja, fechar DM, itens do menu de canal, “Mencionar”, ícones do header Friends, menu “Mais” e descobrir servidores não têm ação. “Mensagem” no menu de membro abre perfil, não DM. No oficial desktop 1272×756, `Add Friend` é botão separado à direita e Active Now não aparece sem conteúdo; no RS, Add Friend fica dentro da tab bar e o painel de 360px é sempre montado.
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

- [ ] **T-049** `[QA-Media]` — **P1** — **QA/Notificações & Presença** — **Suíte de regressão para Notificações, Badges de Unread/Menção, Contadores e Presença em Tempo Real**
  - **Arquivos:** `tests/notifications_badges_and_presence_test.rs` (novo).
  - **Status:** Em execução por `QA-Media`.

- [ ] **T-045** `[QA-Voice]` — **P0** — **QA/Áudio** — **Testes de regressão para PCM/resampling 44.1↔48kHz, jitter/underrun, Opus PLC, mixer multi-speaker e transições mute/deafen/speaking**
  - **Arquivos:** `tests/audio_engine_regression_test.rs` (novo).
  - **Status:** Em execução por `QA-Voice`.

- [ ] **T-046** `[QA-Msgs]` — **P1** — **QA/UX & Menus** — **Testes de regressão para botões/menus inertes (`T-033`, `T-034`), atalhos anunciados (`T-035`) e layout Friends Hub**
  - **Arquivos:** `tests/ux_menus_keybinds_regression_test.rs` (novo).
  - **Status:** Em execução por `QA-Msgs`.
