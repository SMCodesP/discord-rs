# Especificação de Tema por Conta no Discord (Theme per Account)

Este documento descreve como o cliente oficial do Discord recebe as configurações de tema da conta do usuário via Gateway WebSocket, o esquema oficial Protobuf verificado, o tratamento de DMs fixadas e como o `discord-rs` implementa a leitura segura dessas preferências em conformidade com as regras do projeto.

> **Nota de Privacidade**: Nenhuma informação confidencial de conta de usuário (tokens de autenticação, IDs reais, e-mails ou conteúdo privado) é incluída neste documento ou código. Todos os testes utilizam exclusivamente bytes sintéticos construídos no próprio código de teste.
>
> **Fronteira de Somente Leitura (Zero Writes)**: O `discord-rs` opera estritamente em modo de leitura para as configurações de usuário. Nenhuma requisição `PATCH` em endpoints de settings-proto é realizada. Fixações e desfixações manuais de canais pelo usuário continuam operando exclusivamente de forma local no cache (`~/.config/discord-rs/cache/pinned_dms.json`).

---

## 1. Origem dos Dados de Configuração de Tema

O Discord não armazena as preferências modernas de aparência em campos JSON comuns de usuário. Em vez disso, utiliza buffers Protobuf binários codificados em Base64 transmitidos via WebSocket (Gateway) e sincronizados via chamadas REST.

### A. Gateway WebSocket (Evento `READY`)
- **Evento**: `READY` (Opcode 0 - Dispatch)
- **Campo de dados**: `user_settings_proto`
- **Formato**: String Base64 contendo a mensagem serializada Protobuf `PreloadedUserSettings`.

### B. Gateway WebSocket (Evento `USER_SETTINGS_PROTO_UPDATE`)
- **Evento**: `USER_SETTINGS_PROTO_UPDATE`
- Emitido pelo Gateway sempre que o usuário altera configurações em qualquer cliente conectado.
- Payload contém:
  - `settings`: String Base64 contendo `PreloadedUserSettings` atualizado.
  - `partial`: Booleano indicando se o payload contém apenas campos modificados (`true`) ou o snapshot completo (`false`).
  - **Tratamento de parcial**: Quando `partial: true` e o subcampo `appearance` não está presente no buffer, o `discord-rs` **não** apaga o tema já registrado no `AppState`.

### C. Endpoints REST API (Informativo / Sem escrita)
- **Endpoint**: `PATCH /api/v9/users/@me/settings-proto/1`
  - Utilizado pelo cliente oficial web/desktop ao salvar configurações de aparência / tema.
  - **Limite do discord-rs**: Proibido escrever na conta de usuário. Nenhuma chamada de escrita ou mutação é efetuada pelo cliente.
- **Endpoint de consulta**: `GET /api/v9/users/@me/settings-proto/1`
  - Retorna o payload encapsulado: `{"settings": "<base64_string>"}`.

---

## 2. Esquema Oficial Protobuf Verificado (`PreloadedUserSettings.proto`)

Fonte oficial verificada:
Arquivo `/home/smcodes/.cache/discord-rs-parity/orch/PreloadedUserSettings.proto` (pacote `discord_protos.discord_users.v1`), extraído do cliente oficial do Discord pelo projeto `discord-userdoccers/discord-protos`.

### Campos e Linhas Exatas do Esquema:

| Mensagem | Campo | Nº Campo | Tipo de Fio (Wire Type) | Linha no `.proto` | Descrição |
|---|---|---|---|---|---|
| `PreloadedUserSettings` | `appearance` | **13** | 2 (Length-delimited) | Linha 607 | Container de configurações de aparência |
| `AppearanceSettings` | `theme` | **1** | 0 (Varint) | Linha 438 | Enum `Theme` selecionado |
| `AppearanceSettings` | `client_theme_settings` | **3** | 2 (Length-delimited) | Linha 440 | Presets de gradiente e cores customizadas |
| `ClientThemeSettings` | `background_gradient_preset_id` | **2** | 2 (Length-delimited `UInt32Value`) | Linha 406 | ID numérico do preset de gradiente de fundo |
| `ClientThemeSettings` | `custom_user_theme_settings` | **4** | 2 (Length-delimited `CustomUserThemeSettings`) | Linha 407 | Cores personalizadas do gradiente |
| `CustomUserThemeSettings` | `colors` | **1** | 2 (Repeated `string`) | Linha 399 | Lista de cores hexadecimais (ex.: `"#1e1f22"`) |
| `CustomUserThemeSettings` | `gradient_color_stops` | **2** | 5 (Fixed32) ou 2 (Packed `float`) | Linha 400 | Pontos de parada do gradiente |
| `CustomUserThemeSettings` | `gradient_angle` | **3** | 0 (Varint `int32`) | Linha 401 | Ângulo do gradiente em graus |
| `CustomUserThemeSettings` | `base_mix` | **4** | 0 (Varint `int32`) | Linha 402 | Mix base de interpolação |

### Valores do Enum `Theme` (`PreloadedUserSettings.proto:390-396`):
- `THEME_UNSET = 0` (Linha 391)
- `THEME_DARK = 1` (Linha 392)
- `THEME_LIGHT = 2` (Linha 393)
- `THEME_DARKER = 3` (Linha 394)
- `THEME_MIDNIGHT = 4` (Linha 395)

---

## 3. DMs Fixadas: Análise do Esquema e Persistência Local

A auditoria contra `/home/smcodes/.cache/discord-rs-parity/orch/PreloadedUserSettings.proto` demonstrou que:
1. **Ausência no Esquema Protobuf**: Executando busca por `pin` no arquivo `.proto` (`grep -n -i pin PreloadedUserSettings.proto`), não existe **nenhum** campo relativo a canais fixados ou DMs fixadas.
2. **Diferenciação com `Favorites`**: O campo `Favorites favorites = 15;` (linhas 481-486, 609) representa a funcionalidade de "Favoritos" de servidor do Discord (`guild_visible`, `auto_add_joined_threads`, `FavoriteChannel.collapsed`), onde `FavoriteChannel` contém `nickname = 1`, `type = 2`, `position = 3`, `parent_id = 4`, `channel_type = 5`, `collapsed = 6`. Não se trata de fixação de DMs.
3. **Evidência de DMs Fixadas no Discord Oficial**: Nenhuma rota ou campo no `PreloadedUserSettings` armazena DMs fixadas da lista lateral de mensagens diretas no cliente Discord oficial inspecionado.
4. **Decisão Arquitetural no `discord-rs`**:
   - As DMs fixadas permanecem estritamente **locais**.
   - O arquivo de persistência em disco `~/.config/discord-rs/cache/pinned_dms.json` é a autoridade local de ordenação das DMs fixadas no cliente.
   - Nenhuma ligação fictícia de favoritos com pins de DM é mantida.

---

## 4. Integração no `discord-rs` e Próximos Passos

1. **Decodificação no Protocolo**:
   - `src/protocol/user_settings_proto.rs` decodifica com segurança o buffer binário de `user_settings_proto` (Base64) extraindo `appearance.theme` e `appearance.client_theme_settings`.
2. **Armazenamento no Estado**:
   - O tema da conta é mantido em `AppState::account_theme` e `AppState::account_client_theme_settings`.
   - **Não sobrescrever a escolha exibida**: A preferência de UI `AppState::theme_choice` não é alterada automaticamente pelo evento `AccountThemeUpdated` para evitar exibir "Claro" selecionado na interface de configurações quando apenas a paleta escura canônica (`DiscordPalette::dark()`) está implementada.
3. **Próximos Passos (Próxima Fase)**:
   - Quando paletas completas adicionais (ex.: Clara ou Midnight) forem desenhadas com paridade oficial e auditadas, uma camada de renderização poderá ligar `account_theme` a `DiscordExtras::install_with_palette(cx, palette)`.
