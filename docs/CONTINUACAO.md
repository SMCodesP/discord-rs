# Discord RS — Documento de Continuação

> Estado em 2026-10-10, `main` em `a68e0de`, 277 testes passando e 0 falhando
> (`cargo test --no-fail-fast`). Escrito pelo orquestrador ao final da primeira
> rodada, quando as contas do Antigravity ficaram sem cota.
>
> **A próxima rodada deve tentar o preset `OpenCode` do Relay em vez do
> Antigravity** (pedido do dono do projeto). O que isso muda, e o que ainda
> precisa ser confirmado, está na seção 3.0 do documento irmão.
>
> Documento irmão: [`GPUI_KIT_MIGRATION_SCOPE.md`](GPUI_KIT_MIGRATION_SCOPE.md),
> o escopo da migração de GPUI e de componentes para o próximo orquestrador.

---

## 1. Onde paramos

### 1.1 O que está na `main`

| Fase do roadmap | Estado | Observação |
|---|---|---|
| 1. Purga de mocks | Integrada | Só os três itens do roadmap. O resto dos mocks continua (seção 4). |
| 2. Friends Hub | Integrada | Abas, ações reais pela API, "Ativo agora" por presença real. |
| 3. Presença e status nas DMs | Integrada | Indicadores e subtítulo de status. |
| 4. Popout de perfil | Integrada, **com defeito de comportamento** | Abre como modal central (seção 2.1). |
| 5. Decorações e efeitos de perfil | Integrada, **quase sem efeito visível** | A decoração só chega ao popout (seção 2.2). |
| 6. DMs fixadas | Integrada | Recurso local; não sincroniza com o Discord (seção 5.4). |
| 7. Canais de voz de guilda | Integrada | Membros conectados e contador de capacidade. |
| 8. Voz: gateway, UDP, áudio, câmera/tela, sessão, DAVE | Integrada | **Nunca testada contra o Discord real** (seção 3). |
| 8.3 Palco de chamada e botões de chamada | **Não iniciada** | O agente bateu na cota antes de fazer qualquer coisa (seção 6). |

### 1.2 O que ficou aberto no ambiente

- **Agente `Stage`** no canvas, parado com erro de cota e sem nenhuma alteração.
  Pode ser dispensado (`relay dismiss Stage`); a tarefa dele está na seção 6.
- **Layers e branches `strata/*`** de todos os agentes ainda existem. O trabalho
  já está na `main`. Algumas dessas branches guardam no histórico um nome de
  exibição e dois IDs que parecem ter vindo da conta real (troquei por valores
  sintéticos na `main`). Recomendo apagar as branches locais ao final.
- **Chrome compartilhado** aberto com `--remote-debugging-port=9222` e perfil em
  `~/.cache/discord-rs-inspect-chrome`, logado no Discord. Qualquer processo
  local controla esse navegador enquanto ele estiver aberto. Feche quando não
  estiver em uso.
- **Configuração global do Antigravity alterada:**
  `~/.gemini/config/mcp_config.json` (o arquivo das contas do Relay é um link
  para ele) passa `--browserUrl=http://127.0.0.1:9222` ao `chrome-devtools-mcp`.
  Sem aquele Chrome aberto, a ferramenta de navegador falha em qualquer projeto.
  O original está em
  `~/.local/state/relay/accounts/antigravity/smcodes003/.gemini/config/mcp_config.json.bak-before-shared-chrome`.
- **OpenCode preparado em 2026-10-10:** modelo padrão
  `opencode/muse-spark-1.3-contributor-free`, MCP `chrome-devtools` global,
  `AGENTS.md` e `opencode.json` na raiz do projeto. Tudo commitado em
  `f1d96df`. Falta abrir o Chrome da porta 9222 (seções 3.0 e 3.6 do documento
  irmão).

---

## 2. Defeitos visuais apontados pelo dono do projeto

### 2.1 Perfil abre como modal, e deveria abrir como popout ancorado

**Hoje:** clicar em um nome ou avatar abre o perfil no centro da janela, com
fundo escurecido. Em `src/app/user_profile_popout.rs`,
`render_user_profile_popout` monta um container `absolute().inset_0()` com
`bg(rgba(0x00000077))` e `items_center().justify_center()`.

**Como é no Discord:**

1. Clicar no nome ou avatar abre um **popout pequeno ancorado ao elemento
   clicado**, sem escurecer o fundo. Ele fecha ao clicar fora ou com Esc.
2. O **modal de perfil completo** (maior, centralizado, com fundo escurecido) só
   abre quando se clica no avatar ou no nome **dentro do popout**.

**O que fazer:**

- Separar em dois componentes: `UserProfilePopout` (ancorado) e
  `UserProfileModal` (completo). Os dois consomem o mesmo `UserProfileData`.
- Guardar no estado a posição ou o elemento de origem do clique. Hoje
  `AppState::open_user_profile(user_id)` não recebe posição.
- Ancorar com `deferred(anchored().position(..).snap_to_window())`, como o menu
  de contexto de fixar DM já faz em `src/app/channel_sidebar.rs`.
- Pontos de abertura a atualizar: `src/app/member_sidebar.rs:112`,
  `src/app/friends_hub.rs:1189` e `:1270`, `src/app/channel_sidebar.rs:554`,
  `src/chat/message_list.rs:776` e `:817`.
- Medir os dois no Discord real (largura, posição relativa ao elemento, lado
  para onde abre perto das bordas) antes de codificar.

### 2.2 Decorações de avatar não aparecem na lista de DMs nem na conversa

**Causa:** a decoração só foi ligada em dois lugares.

| Lugar | Decoração chega? |
|---|---|
| Popout de perfil (`user_profile_popout.rs:244`) | Sim |
| `MemberItem` (`src/components/member_item.rs`) | Sim, mas **esse componente não é usado em lugar nenhum** |
| Lista de DMs, conversa, Friends Hub, lista de membros, canais de voz, painel do usuário, "Ativo agora", palco de chamada | Não |

Dois problemas se somam:

1. **Quase ninguém usa o componente `Avatar`.** `Avatar::new` aparece em 3
   arquivos; o resto desenha o avatar à mão com `img(...).rounded_full()`. A
   conversa usa uma função própria, `render_avatar`, em
   `src/chat/message_list.rs:403`.
2. **O dado não chega aos modelos de interface.** `avatar_decoration_data` só é
   lido em `parse_user_from_json` (`src/protocol/discord_api.rs:1764`) e existe
   em `domain::User`. Não existe em `Author` (`src/domain/message.rs`), nem em
   `FriendItemState`, `ChannelItemState`, `MemberItemState`, `ActiveNowItem`,
   `VoiceParticipantState` ou no usuário atual (`src/app/state.rs`).

**Requisito do dono do projeto:** em todo lugar onde aparece a foto de um
usuário do Discord, a decoração dele aparece junto.

**O que fazer:**

- Levar `avatar_decoration_data` a todos os modelos acima e a todos os pontos de
  parsing: autor de mensagem, destinatários de DM, relacionamentos, membros de
  guilda, `member` dentro de voice state, usuário do READY.
- Trocar todo avatar desenhado à mão pelo componente `Avatar`, que já sabe
  desenhar decoração e status. É o mesmo trabalho da frente "Avatar único" do
  documento de GPUI; não faça duas vezes.
- Conferir no Discord real o tamanho da moldura em cada contexto (24, 32, 40 e
  80 px) e que o indicador de status fica por cima dela.

### 2.3 Item cortado na mensagem

O pedido original continha um trecho que chegou cortado: *"quero também que
nesse escopo acrescente o seguinte ção local"*. Não foi possível recuperar o que
vinha antes de "ção local". **Confirmar com o dono do projeto antes de começar.**
Um palpite, sem base além do contexto: sincronização local, ou seja, as DMs
fixadas, que hoje são só locais (seção 5.4).

---

## 3. Voz: o que existe e o que nunca foi provado

A pilha inteira compila e tem testes locais, mas **nenhuma parte foi testada
contra o servidor do Discord**.

| Peça | Arquivo | Limite conhecido |
|---|---|---|
| Voice Gateway v8 | `src/protocol/voice_gateway.rs` | Códigos de fechamento classificados; sem teste real. |
| UDP, RTP, criptografia | `src/protocol/voice_udp.rs`, `voice_crypto.rs` | Testes cifram e decifram com o próprio código: provam consistência, não interoperabilidade. |
| DAVE (E2EE) | `src/protocol/voice_dave.rs` | Usa o crate `davey` 0.1.4. **Sem teste ponta a ponta:** o crate não permite formar um grupo entre dois clientes sem o servidor. |
| Sessão de voz | `src/protocol/voice_session.rs` | Revisada com menos profundidade que o resto. |
| Áudio | `src/media/` | `cpal` + Opus. Não confirmado se `cpal::Stream` é `Send` no Linux; manter o motor em thread dedicada. |
| Câmera e tela | `src/capture/` | PipeWire e X11 implementados; **nenhum quadro real visto**. |

**DAVE é obrigatório.** A documentação oficial diz que desde 1º de março de 2026
o Discord só aceita chamadas com E2EE em DMs, grupos, canais de voz e Go Live.
Sem DAVE funcionando, o servidor recusa com o código 4017.

**Vídeo remoto não existe.** Não há codificação nem decodificação de vídeo
(H264/VP8) nem empacotamento RTP de vídeo. A câmera e a tela só são capturadas
localmente.

### Testes manuais que só o dono do projeto pode fazer

```
cargo run --example screen_capture_probe -- /tmp/tela.png
cargo run --example camera_probe -- /tmp/camera.png
cargo run --example voice_call_probe -- <id_do_canal> [id_do_servidor]
```

- O primeiro abre o diálogo do sistema para escolher a tela e grava um PNG.
- O terceiro entra num canal de voz com o token que o app já guarda e imprime
  cada etapa, incluindo o estado do DAVE e o código de erro se cair. Use um
  canal de teste.

Os três compilam; nenhum foi executado. **Rode-os antes de investir no palco de
chamada:** se a chamada real falhar, o problema está na pilha, não na interface.

---

## 4. Mocks que continuam no projeto

A Fase 1 removeu só três itens. A política de zero mocks ainda é violada em:

| Mock | Onde |
|---|---|
| Mensagens de seed | `src/app/state.rs:497` (`generate_seed_messages`) |
| Guildas de seed "rust_brasil" e "gpui_hub", com canais, emojis e stickers | `src/app/state.rs:715`, `:879`, `:1038` |
| Amigo e solicitações de DM falsos, membros falsos | `src/app/state.rs`, bloco de inicialização do `AppState` |
| Nitro, boosts, conexões, apps autorizados, Family Center falsos | `src/app/state.rs:1195` em diante; `:2114` |
| Ping fixo `ping_ms: 18` | `src/app/state.rs:2206` (sai junto com o palco de chamada) |
| Login falso (`mock_`, `test`, `demo`) e "Modo Demonstração (Offline)" | `src/app/state.rs:2442`, `:1069`, `:2500` |
| `MockDiscordProvider` | `src/protocol/mock_provider.rs` (824 linhas), usado por `client_manager.rs` e por testes |
| Tela de configurações | `src/app/user_settings.rs` (4694 linhas), que consome boa parte dos dados falsos acima |

Remover o `MockDiscordProvider` quebra `tests/protocol_tests.rs`,
`tests/dm_messaging_test.rs`, `tests/e2e_full_flow.rs` e outros. É uma tarefa
própria: o provedor de teste precisa ir para `tests/` ou para `#[cfg(test)]`, e
o app sem login precisa mostrar a tela de login, não um servidor de exemplo.

Um efeito colateral já apareceu: um amigo de seed marcado como online fez um
relacionamento real herdar presença falsa. Enquanto houver seed, esse tipo de
bug volta.

---

## 5. Outras pendências

### 5.1 Efeitos de perfil sem animação temporal
`src/components/profile_effect.rs` desenha as camadas reais do catálogo, mas
todas de uma vez. Os campos `start`, `duration`, `loop` e `loopDelay` são lidos
e ignorados.

### 5.2 Envio rápido no popout
Existe e está ligado ao Enter (`send_profile_popout_quick_message`), mas não foi
lido por dentro nem testado.

### 5.3 "Ativo agora"
Usa presença real. Faltam a imagem da aplicação e a capa do álbum quando o
payload traz `assets`, e a barra de progresso do Spotify.

### 5.4 DMs fixadas são locais
O Discord oficial guarda as DMs fixadas num protobuf de configurações
(`PreloadedUserSettings`) que o projeto não lê. Hoje a lista fica em
`~/.config/discord-rs/cache/pinned_dms.json` e não sincroniza com a conta.
Sincronizar exige ler e escrever esse protobuf.

### 5.5 Teste instável
`tests/cache_and_ratelimit_test.rs::test_pillar_4_instant_startup_0ms_blank_screen`
exige menos de 50 ms e falha com a máquina carregada (levou 71 ms uma vez). Não
foi alterado.

### 5.6 `state.rs` grande demais
`src/app/state.rs` tem 3737 linhas e foi tocado por quase todos os agentes. Foi
a maior fonte de conflito de merge. Vale quebrar em módulos antes da próxima
rodada paralela.

---

## 6. Tarefa pendente: palco de chamada (Fases 8.1 e 8.3)

O agente `Stage` (Antigravity) recebeu esta tarefa e parou por cota sem alterar
nada. Dispense-o e recrute um agente novo com `--preset OpenCode`. A cópia
dele é a branch `strata/stage-aa1fcfba`, igual à `main`.

**Arquivos:** `src/app/voice_call_panel.rs` (reescrever), `src/app/header_bar.rs`
(botões), `src/app/state.rs` (só o estado da chamada), `src/app/root_view.rs`
(montar o palco), `src/i18n/*`, `tests/`.

**O que fazer:**

1. **Ligar a chamada de verdade.** `connect_voice` hoje só cria um
   `ActiveVoiceCall` local. Trocar por uma `VoiceSession` iniciada pelo
   `client_manager`, com os estados Connecting, Connected, Reconnecting e
   Disconnected refletidos na tela. Remover o ping fixo: mostrar latência só com
   medida real. Erros fatais aparecem com a descrição real.
2. **Participantes reais:** de `voice_states_by_channel`. "Falando" só com
   evento real da sessão.
3. **Palco:** grade responsiva de tiles, anel verde de 2 px em quem fala, modo
   foco para compartilhamento de tela. Vídeo local pode vir de `src/capture/`;
   vídeo remoto não existe, então mostra o avatar, nunca um vídeo falso.
4. **Barra de controles:** mudo e câmera com seta que lista os dispositivos
   reais do `MediaDeviceManager` e troca sem derrubar a chamada; compartilhar
   tela; desconectar em `#da373c`.
5. **Cabeçalho de DM:** botões de chamada de voz e de vídeo, que chamam
   `ring_call` e abrem o palco.

`examples/voice_call_probe.rs` é um uso completo e real da `VoiceSession`.

**Recomendação:** fazer esta tarefa **depois** da migração de GPUI, ou então já
em cima dos componentes novos. Reescrever o palco agora e de novo na migração é
trabalho dobrado.

---

## 7. Ordem sugerida

1. Rodar os três testes manuais da seção 3. O resultado decide se a pilha de voz
   precisa de correção antes de qualquer interface.
2. Confirmar o item cortado da seção 2.3.
3. Executar a migração de GPUI conforme
   [`GPUI_KIT_MIGRATION_SCOPE.md`](GPUI_KIT_MIGRATION_SCOPE.md). As seções 2.1 e
   2.2 deste documento já estão incluídas lá como frentes.
4. Palco de chamada (seção 6), sobre os componentes novos.
5. Purga total de mocks (seção 4).
6. Pendências menores da seção 5.
