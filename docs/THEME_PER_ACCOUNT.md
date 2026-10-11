# Especificação de Tema por Conta no Discord (Theme per Account)

Este documento descreve como o cliente oficial do Discord recebe e persiste as configurações de tema da conta do usuário via Gateway e API REST, e o que é necessário para suportar esse fluxo no `discord-rs`.

> **Nota de Privacidade**: Nenhuma informação confidencial da conta inspecionada (tokens de autenticação, IDs de usuário, cores ou hashes de tema personalizados) é incluída neste documento.

---

## 1. Origem dos Dados de Configuração de Tema

O Discord não armazena as preferências modernas de aparência em campos JSON comuns de usuário. Em vez disso, utiliza buffers Protobuf binários codificados em Base64 transmitidos via WebSocket (Gateway) e sincronizados via chamadas REST.

### A. Gateway WebSocket (Evento `READY`)
- **Evento**: `READY` (Opcode 0 - Dispatch)
- **Campo de dados**: `user_settings_proto`
- **Formato**: String Base64 contendo a mensagem serializada Protobuf `PreloadedUserSettings`.
- **Evento de atualização**: `USER_SETTINGS_PROTO_UPDATE`
  - Emitido pelo Gateway sempre que o usuário altera configurações em qualquer cliente conectado.
  - Payload contém o campo `settings` com a string Base64 atualizada e `partial` (booleano).

### B. Endpoints REST API
- **Endpoint**: `PATCH /api/v9/users/@me/settings-proto/1`
  - Utilizado pelo cliente oficial ao salvar configurações de aparência / tema.
  - O sufixo `/1` indica o tipo de proto: `1` para `PreloadedUserSettings` (configurações gerais), `2` para `FrecencyUserSettings`.
  - Corpo da requisição: `{"settings": "<base64_string>"}`.
- **Endpoint de consulta**: `GET /api/v9/users/@me/settings-proto/1`
  - Retorna o payload encapsulado: `{"settings": "<base64_string>"}`.

---

## 2. Esquema da Mensagem Protobuf (`PreloadedUserSettings`)

O buffer binário decodificado corresponde à estrutura `discord_protos.discord_users.v1.PreloadedUserSettings`:

### Campos Relevantes para Tema:
1. **`appearance`** (Mensagem `AppearanceSettings`):
   - `theme`: Enum / string de tema básico (`THEME_DARK`, `THEME_LIGHT`, `THEME_MIDNIGHT`).
2. **`client_theme_settings`** (Mensagem `ClientThemeSettings`):
   - `background_gradient_preset_id`: Identificador numérico do preset de gradiente selecionado pelo usuário (quando aplicável).
   - `custom_theme_colors`: Lista / array de inteiros de 32 bits (representando cores em formato numérico `0xRRGGBB` para os pontos de interpolação do gradiente de fundo).

*Status de verificação em inspeção*:
- Nomes dos endpoints: Confirmados (`/api/v9/users/@me/settings-proto/1`, evento `READY.user_settings_proto`).
- Nomes dos campos: `appearance.theme`, `client_theme_settings.background_gradient_preset_id`, `client_theme_settings.custom_theme_colors`.
- Valores específicos da conta: *Omitidos intencionalmente por política de zero vazamento*.

---

## 3. Lacunas no `src/protocol` do `discord-rs`

Atualmente, o `src/protocol` do `discord-rs` não consegue desserializar nem aplicar temas por conta:

1. **Parser de Protobuf**:
   - O projeto utiliza `serde_json` para payloads do Gateway e REST.
   - Não há dependência de decodificador Protobuf (ex.: `prost` ou `protobuf-codegen`) no `Cargo.toml`.
2. **Decodificação Base64**:
   - É necessário integrar a decodificação da string Base64 de `user_settings_proto` para bytes antes da desserialização do protobuf.
3. **Tratamento no Gateway Dispatch**:
   - Em `src/protocol/discord_api.rs:1361-1405`, o manipulador do evento `READY` tenta extrair `user_settings` como objeto JSON legado. O campo `user_settings_proto` é ignorado.
   - O evento `USER_SETTINGS_PROTO_UPDATE` não possui manipulador em `src/protocol/discord_api.rs`.
4. **Clientes REST**:
   - Não existem rotas mapeadas em `src/protocol/discord_api.rs` para interagir com `/api/v9/users/@me/settings-proto/1`.
5. **Integração com o Sistema de Tokens**:
   - A infraestrutura de tokens em `src/tokens/` agora está preparada via `DiscordPalette` e `install_with_palette(cx, palette)`.
   - Quando o `src/protocol` for capaz de ler `PreloadedUserSettings`, ele poderá instanciar uma `DiscordPalette` customizada e chamar `DiscordExtras::install_with_palette(cx, palette)`.
