# UI Shot — captura determinística de telas (Etapa 2, item 3)

Ferramenta para fotografar o app numa tela específica, em estado
determinístico, **sem conta real e sem rede**. Sem ela nenhuma tela pode ser
dada como pronta.

## Uso

```sh
scripts/ui-shot.sh <tela> <saida.png> [LARGURAxALTURA]
```

- `<tela>`: `login` `friends` `dm` `guild` `settings` `profile_popout`
  `media_picker` `search` (uma fixture em `tests/fixtures/ui_shot/<tela>.json`).
- `<saida.png>`: caminho do PNG. Um nome simples (sem `/`) cai no padrão
  **fora do repo**: `~/.cache/discord-rs-parity/app/`.
- `[LARGURAxALTURA]`: padrão `1280x800`.

Exemplo:

```sh
scripts/ui-shot.sh guild ~/.cache/discord-rs-parity/app/guild.png 1280x800
```

O script compila `examples/ui_shot`, abre o app sob um Xvfb privado, espera
o primeiro frame desenhado, captura com `import -window <id>` e **falha**
se o PNG final não tiver exatamente o tamanho pedido ou estiver em branco.
O processo do app e o Xvfb são sempre mortos ao final (sem órfãos).

## Como funciona

1. `examples/ui_shot <fixture.json> <titulo> <LARGURAxALTURA>` monta o
   `AppState` só com APIs públicas (`AppState::new()` + campos `pub`, e
   `RootView::with_state` em vez de `RootView::new` — ver § "Sem rede").
   A fixture **substitui** o seed de demonstração: as coleções são limpas
   antes de aplicar o conteúdo sintético.
2. A janela usa o `<titulo>` único da execução; o script a localiza com
   `xdotool search --name` e fotografa com `import -window <id> out.png`.
3. Timestamps das mensagens são determinísticos: base fixa
   `2026-01-05T12:00:00Z` menos `minutes_ago` da fixture.

## Sem rede e sem disco do usuário

- O example aponta `XDG_CONFIG_HOME` para um diretório temporário **antes**
  de construir qualquer estado, e remove `DISCORD_TOKEN` /
  `DISCORD_TOKEN_IS_USER` do ambiente (o script faz o mesmo).
- `auth_token` e `client_manager` são forçados a `None` depois do
  `AppState::new()` (sem auto-login, sem gateway); `pinned_dms_path = None`
  desliga persistência; `save_token_to_disk = false`.
- As fixtures não contêm nenhuma URL (sem avatar/ícone/anexo remoto), então
  o pipeline de mídia/imagem não tem o que buscar.

Caminhos de rede que **não** executam (auditoria em `src/`, que este
harness não toca):

| Caminho | Arquivo:linha | Por que não roda |
|---|---|---|
| Auto-login com token salvo (`connect_with_token` via `tokio::spawn`) | `src/app/root_view.rs:40-61` (`RootView::new`) | o example usa `RootView::with_state` (`src/app/root_view.rs:109-139`), que não cria `ClientManager` nem agenda login |
| Leitura de token em disco (`TokenStorage::load`) e de cache de guilds (`load_guilds_from_disk`) | `src/app/state.rs:1028-1034` (dentro de `AppState::new`) | `XDG_CONFIG_HOME` temporário: arquivos não existem → `Ok(None)`/`None`; e o resultado é descartado (`auth_token = None`) |
| Envio de friend request / aceitar / remover (`ClientManager::send_friend_request` etc., via `tokio::spawn`) | `src/app/state.rs:1345`, `1378`, `1400` | só disparam em clique do usuário; `client_manager` é `None` e nada é clicado |
| `ReqwestHttpClient` (HTTP real) | `src/main.rs` / `src/protocol/http_client.rs` | a instância é registrada no platform como no `main`, mas nenhum request é feito: sem URLs nas fixtures e sem ação de rede |
| Telemetria com export (`TelemetryService::start`) | `src/telemetry/mod.rs:178` | o example não inicia o serviço; usa só o `global()` em memória herdado do `AppState::new` |

## Limitações conhecidas

- **Display**: o padrão é um Xvfb privado (tamanho exato, sem interferência
  do compositor). Sob o COSMIC em `:0` (`UI_SHOT_DISPLAY=:0`) o compositor
  retileja a janela (observado: pediu 1280x800, veio 846x1374 e depois
  1698x1374; `xdotool windowsize` ignorado) e o script falha no check de
  tamanho — de propósito, para nunca entregar PNG fora do padrão.
- **Aba do media picker**: só `Emojis` é capturável offline. `Stickers`,
  `Gifs` e `Soundboard` (abas em `src/chat/media_picker.rs:48-54`) podem
  tentar carregar previews remotos; a fixture só declara `Emojis`.
- **Anexos**: só arquivos de texto (`.txt`/`.rs`, `text/plain`), que
  renderizam como cartão de arquivo (`src/chat/message_list.rs:492-531`).
  Anexos de imagem com URL remota nunca são usados (sem rede).
- **Achados no código de produção (fora do escopo: `src/` intocado)**:
  - `src/app/user_settings.rs:525` — e-mail mockado fixo
    (`vanguard@discord-rs.native`) na aba Minha Conta; a fixture não tem
    API pública para sobrescrevê-lo, então ele aparece na foto de
    `settings`.
  - `src/app/user_profile_popout.rs:720` — servidor em comum cai no
    fallback `Servidor {id}` quando `nick` é `None` (sem lookup do nome da
    guild); aparece como "Servidor g1" na foto de `profile_popout`.
- Janela usa `gpui::init(cx)` + `gpui::open_window`
  (`gpui-kit-0.7.1/src/lib.rs:149-180`), igual ao `main.rs` pós-migração.
