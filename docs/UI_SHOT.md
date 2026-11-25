# UI Shot & UI Compare — Captura Determinística e Comparação Visual

Conjunto de ferramentas para fotografar o app numa tela ou sub-estado específico,
em estado determinístico, **sem conta real e sem rede**, e comparar as capturas
lado a lado com as referências visuais oficiais do Discord inspecionado.

Sem essas ferramentas, nenhuma alteração de interface pode ser validada visualmente.

---

## 1. Scripts e Uso

### `scripts/ui-shot.sh`

```sh
scripts/ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]
```

- `<tela>`: Identificador da tela ou sub-estado (corresponde a uma fixture em `tests/fixtures/ui_shot/<tela>.json`).
- `<saida.png>`: Caminho de saída da imagem. Nome sem `/` vai para `~/.cache/discord-rs-parity/app/`. A saída **nunca** pode ficar dentro do repositório.
- `[LARGURAxALTURA]`: Dimensão da janela e do PNG capturado. Padrão: `1280x800`.

#### Variáveis de Ambiente

- `UI_SHOT_CARGO_LOCK`: lock flock para cargo (padrão `/tmp/discord-rs-cargo-B.lock`).
- `UI_SHOT_DISPLAY`: reutiliza display existente (ex: `:0`) em vez de Xvfb privado; tiling pode reprovar a checagem de tamanho exato.
- `UI_SHOT_XVFB_DISPLAY`: display preferencial do Xvfb (padrão `:99`).
- `UI_SHOT_CAPTURE_TIMEOUT_SECS`: timeout por tentativa de captura (padrão `15`).
- `UI_SHOT_READY_TIMEOUT_SECS`: espera máxima por primeiro frame não preto (padrão `20`).

#### Como Funciona

1. Compila `examples/ui_shot` com lock de cargo.
2. Sobe `Xvfb` privado (ou usa `UI_SHOT_DISPLAY`) com tela maior que a janela.
3. Inicia o app isolado (`XDG_CONFIG_HOME` efêmero, sem `DISCORD_TOKEN*`, sem `WAYLAND_DISPLAY`) com título único.
4. Aguarda janela via `xdotool`, força tamanho exato e espera primeiro frame renderizado.
5. Captura com `import -window <id>` e timeout; em falha usa fallback `xwd -silent -id ... | magick xwd:- ...` com timeout/log de causa.
6. Valida PNG final: dimensões exatas, arquivo não vazio, `stddev > 0.02` (rejeita imagem preta/vazia/uniforme). Falha explicitamente em timeout/captura preta.
7. Gera sidecar `<saida>.meta.json` (schema `ui-shot-meta-v1`) com viewport, DPR, título, rota/fixture, tamanho real, stddev, sha256, backend e causa; sem IDs/nomes/conteúdo real.
8. Encerra app e Xvfb no `EXIT`.

---

### `scripts/ui-compare.sh`

```sh
scripts/ui-compare.sh <discord.png> <app.png> <saida.png>
```

- `<discord.png>`: referência (ex.: captura oficial).
- `<app.png>`: captura do app.
- `<saida.png>`: montagem de comparação; nome sem `/` vai para `~/.cache/discord-rs-parity/app/`. Saída **sempre fora do repositório**.

#### Saída e Métricas

1. Exige dimensões idênticas; falha se qualquer imagem for preta/vazia (`stddev <= 0.02`).
2. Executa `compare -metric AE` para contar pixels diferentes.
3. Gera montagem horizontal `[referência] [app] [diff]` e compatibilidade `<prefixo>-lado-a-lado.png` / `<prefixo>-diff.png`.
4. Gera `<saida>.meta.json` (schema `ui-compare-meta-v1`) com size, AE, percent, sha256/stddev das entradas e artefatos; sem dados pessoais.
5. Imprime métricas:
   ```
   ui-compare: discord=<discord.png> app=<app.png> size=1280x800
   ui-compare: AE=<pixels> pixels diferentes de 1024000 (<porcentagem>%)
   ui-compare: montagem: <saida.png>
   ```

### `scripts/ui-shot-selftest.sh`

```sh
scripts/ui-shot-selftest.sh
```

Teste sintético (sem Chrome/browser): gera par comparável da mesma fixture com checksum/dimensões iguais, valida falha explícita em imagem preta e em timeout de captura, e verifica que metadados não contêm chaves de dados pessoais.

---

## 2. Telas e Fixtures Disponíveis

Todas as fixtures residem em `tests/fixtures/ui_shot/<tela>.json` com dados 100% sintéticos (sem dados de usuários ou servidores reais).

### A. Galeria de Componentes Próprios (`gallery`)
Tela de comparação visual de componentes próprios sem Modal, montada lado a lado:
- **Button**: 4 variantes (`Blurple`, `Gray`, `Danger`, `Ghost`) x 3 tamanhos (`Sm`, `Md`, `Lg`) x 2 estados (`normal`, `disabled`) x com e sem ícone (comparação direta com `button-*.png`).
- **IconButton**: 4 estados (`normal`, `active`, `danger`, `disabled`).
- **TextInput**: 4 estados (`vazio com placeholder`, `preenchido`, `com erro`, `secreto / token mascarado`).
- **Banner**: 4 variantes (`Info`, `Success`, `Warning`, `Danger`).
- **Avatar**: 4 tamanhos (`Px80`, `Px40`, `Px32`, `Px24`), com e sem status (`online`, `idle`, `dnd`, `offline`, sem status) e com e sem decoração de avatar.

### B. Telas Principais do App
- `login`: Tela cheia obrigatória de login quando desconectado.
- `friends`: Hub de amigos (aba Disponível/Online; alias `friends_online`).
- `dm`: Visualização de mensagens diretas e chat privado.
- `guild`: Visualização de canal de texto em servidor com lista de membros.
- `settings`: Modal de configurações do usuário (aba Minha Conta; alias `settings_myaccount`).
- `profile_popout`: Popout de perfil de usuário com bio, cargos e status.
- `media_picker`: Popover seletor de mídia com aba de emojis sintéticos.
- `search`: Painel lateral de busca de mensagens com contexto antes/depois.

### C. Sub-estados de Amigos (Abas)
- `friends_online`: Aba "Disponível".
- `friends_all`: Aba "Todos".
- `friends_pending`: Aba "Pendente" com solicitações recebidas e enviadas.
- `friends_add`: Aba "Adicionar amigo" com formulário de solicitação.

### D. Sub-estados de Configurações (Abas de `UserSettings`)
- `settings_myaccount`: Minha Conta.
- `settings_profiles`: Perfis.
- `settings_privacy`: Privacidade & Segurança.
- `settings_authorizedapps`: Aplicativos Autorizados.
- `settings_connections`: Conexões.
- `settings_nitro`: Nitro.
- `settings_serverboost`: Impulso de Servidor.
- `settings_familycenter`: Centro da Família.
- `settings_appearance`: Aparência.
- `settings_accessibility`: Acessibilidade.
- `settings_voicevideo`: Voz & Vídeo.
- `settings_notifications`: Notificações.
- `settings_keybinds`: Atalhos de Teclado.
- `settings_language`: Idioma.

### E. Modais de `ActiveModal`
- `modal_addserver` (alias `modal_add_server`): Diálogo de criar ou entrar em servidor.
- `modal_explore` (alias `modal_explore_servers`): Explorar servidores públicos.
- `modal_login`: Modal interno de troca de token.
- `modal_logindiscord` (alias `modal_login_discord`): Modal de autenticação Discord.
- `modal_addfriendtogroup` (alias `modal_add_friend_to_group`): Diálogo para adicionar amigo à conversa em grupo.
- `modal_creategroupdm` (alias `modal_create_group_dm`): Diálogo para criar mensagem direta em grupo.

---

## 3. Garantias de Isolamento Offline

Para garantir testes reproduzíveis e prevenir vazamento de dados ou chamadas de rede:
- `XDG_CONFIG_HOME` é apontado para diretório temporário exclusivo antes de carregar o estado.
- Variáveis `DISCORD_TOKEN` e `DISCORD_TOKEN_IS_USER` são removidas do ambiente.
- `AppState` é montado via `RootView::with_state` sem instanciar `ClientManager`, sem conexões WebSocket ou spawn assíncrono de auto-login.
- O tema oficial do kit é instalado na mesma ordem que a aplicação de produção: `gpui::init(cx)` seguido de `discord_rs::tokens::kit_theme::install(cx)`.
- Timestamps das mensagens são ancorados num horário fixo base (`2026-01-05T12:00:00Z`).
- Capturas e metadados são gravados fora do repositório; metadados registram apenas viewport/DPR/título/rota/fixture e checksums, sem IDs, nomes, avatares ou conteúdo real.
