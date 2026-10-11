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
- `<saida.png>`: Caminho de saída da imagem. Se for informado apenas um nome de arquivo (sem `/`), o script salva automaticamente fora do repositório em `~/.cache/discord-rs-parity/app/<saida.png>`.
- `[LARGURAxALTURA]`: Dimensão da janela e do PNG capturado. O padrão obrigatório é `1280x800`.

#### Variáveis de Ambiente Suportadas

- `UI_SHOT_CARGO_LOCK`: Caminho do lock flock para cargo (padrão `/tmp/discord-rs-cargo-B.lock` na worktree correspondente).
- `UI_SHOT_DISPLAY`: Reutiliza display existente (ex: `:0`) em vez de iniciar um Xvfb privado. **Aviso**: compositores com auto-tiling podem redimensionar a janela e reprovar a validação estrita de tamanho.
- `UI_SHOT_XVFB_DISPLAY`: Display preferencial do Xvfb (padrão `:99` ou randômico se ocupado).

#### Como Funciona

1. Compila o binário de captura `examples/ui_shot` (armazenado em `target/debug/examples/ui_shot` da worktree, com suporte a `target` como link simbólico).
2. Sobe um servidor `Xvfb` privado com dimensões expandidas (+120px) para acomodar a janela sem recorte.
3. Inicia o app isolado (`XDG_CONFIG_HOME` efêmero, sem variáveis de token) com título de janela exclusivo.
4. Localiza a janela via `xdotool`, ajusta o tamanho exato e aguarda o primeiro frame desenhado (verificado via desvio padrão de cores `identify -format "%[fx:standard_deviation]"` > 0.02).
5. Captura a janela com ImageMagick `import -window <id>` e valida que a resolução é rigorosamente a solicitada e que a imagem não está em branco.
6. Encerra o processo do app e o Xvfb (sem processos órfãos).

---

### `scripts/ui-compare.sh`

```sh
scripts/ui-compare.sh <discord.png> <app.png> <saida.png>
```

- `<discord.png>`: Imagem de referência oficial do Discord (geralmente em `~/.cache/discord-rs-parity/discord/`).
- `<app.png>`: Imagem capturada pelo cliente nativo (gerada pelo `ui-shot.sh`).
- `<saida.png>`: Caminho de saída da imagem de comparação. O script exige que a saída seja **sempre fora do repositório** (rejeita qualquer caminho interno).

#### Saída e Métricas

1. Exige que ambas as imagens tenham as mesmas dimensões (ex.: 1280x800).
2. Executa `compare -metric AE` do ImageMagick para calcular a diferença absoluta de pixels (`AE`) e a porcentagem de pixels divergentes.
3. Monta horizontalmente uma imagem com as 3 etapas lado a lado:
   `[Referência Discord] | [Captura discord-rs] | [Diferença (diff)]`
4. Imprime no terminal a métrica calculada:
   ```
   ui-compare: discord=<discord.png> app=<app.png> size=1280x800
   ui-compare: AE=<pixels> pixels diferentes de 1024000 (<porcentagem>%)
   ui-compare: montagem: <saida.png>
   ```

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
