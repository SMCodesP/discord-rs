# Especificação de Tema por Conta no Discord (Theme per Account)

Este documento descreve como o cliente oficial do Discord recebe as configurações de tema e DMs fixadas da conta do usuário via Gateway WebSocket, o esquema oficial Protobuf verificado, e como o `discord-rs` implementa a leitura segura dessas preferências em conformidade com as regras do projeto.

> **Nota de Privacidade**: Nenhuma informação confidencial da conta inspecionada (tokens de autenticação, IDs de usuário, cores ou hashes de tema personalizados) é incluída neste documento. Todos os testes utilizam exclusivamente bytes sintéticos construídos no próprio código de teste.
>
> **Fronteira de Somente Leitura (Zero Writes)**: O `discord-rs` opera estritamente em modo de leitura para as configurações de usuário. Nenhuma requisição `PATCH` em endpoints de settings-proto é realizada. Fixações e desfixações manuais pelo usuário no cliente continuam operando localmente no cache (`pinned_dms.json`).

---

## 1. Origem dos Dados de Configuração de Tema e Favoritos

O Discord não armazena as preferências modernas de aparência em campos JSON comuns de usuário. Em vez disso, utiliza buffers Protobuf binários codificados em Base64 transmitidos via WebSocket (Gateway) e sincronizados via chamadas REST.

### A. Gateway WebSocket (Evento `READY`)
- **Evento**: `READY` (Opcode 0 - Dispatch)
- **Campo de dados**: `user_settings_proto`
- **Formato**: String Base64 contendo a mensagem serializada Protobuf `PreloadedUserSettings`.
- **Evento de atualização**: `USER_SETTINGS_PROTO_UPDATE`
  - Emitido pelo Gateway sempre que o usuário altera configurações em qualquer cliente conectado.
  - Payload contém o campo `settings` com a string Base64 atualizada e `partial` (booleano).

### B. Endpoints REST API (Informativo / Não implementado para escrita)
- **Endpoint**: `PATCH /api/v9/users/@me/settings-proto/1`
  - Utilizado pelo cliente oficial ao salvar configurações de aparência / tema.
  - O sufixo `/1` indica o tipo de proto: `1` para `PreloadedUserSettings` (configurações gerais), `2` para `FrecencyUserSettings`.
  - **Limite do discord-rs**: Proibido escrever na conta de produção. Nenhuma chamada de escrita ou mutação é efetuada pelo cliente.
- **Endpoint de consulta**: `GET /api/v9/users/@me/settings-proto/1`
  - Retorna o payload encapsulado: `{"settings": "<base64_string>"}`.

---

## 2. Esquema Oficial Protobuf Verificado (`PreloadedUserSettings`)

Fonte oficial verificada:
- Repositório `discord-userdoccers/discord-protos` (`PreloadedUserSettings.proto`, `PreloadedUserSettings.ts`).
- Documentação da comunidade em `https://docs.discord.food/resources/user-settings-proto`.

### A. Mensagem Raiz `PreloadedUserSettings`
- **Campo 13** (`appearance`): Mensagem `AppearanceSettings` (Length-delimited, wire type 2).
- **Campo 15** (`favorites`): Mensagem `Favorites` (Length-delimited, wire type 2).

### B. Mensagem `AppearanceSettings` (Campo 13 de `PreloadedUserSettings`)
- **Campo 1** (`theme`): Enum numérico `Theme` (Varint, wire type 0):
  - `0`: `UNSET`
  - `1`: `DARK` (Padrão Discord Escuro)
  - `2`: `LIGHT` (Claro)
  - `3`: `DARKER` (Escuro Aprofundado)
  - `4`: `MIDNIGHT` (OLED Puro)
- **Campo 3** (`client_theme_settings`): Mensagem `ClientThemeSettings` (Length-delimited, wire type 2).

### C. Mensagem `ClientThemeSettings` (Campo 3 de `AppearanceSettings`)
- **Campo 2** (`background_gradient_preset_id`): Wrapper `google.protobuf.UInt32Value` (Length-delimited, contendo campo 1 com uint32 varint). Identificador numérico do preset de gradiente selecionado.
- **Campo 4** (`custom_user_theme_settings`): Mensagem `CustomUserThemeSettings` (Length-delimited, wire type 2).
  *(Nota de correção: O documento anterior supunha incorretamente uma lista de inteiros de 32 bits `custom_theme_colors`; o esquema oficial verificado utiliza a mensagem aninhada abaixo).*

### D. Mensagem `CustomUserThemeSettings` (Campo 4 de `ClientThemeSettings`)
- **Campo 1** (`colors`): `repeated string` (Length-delimited). Cores hexadecimais (ex.: `"#1e1f22"`).
- **Campo 2** (`gradient_color_stops`): `repeated float` (Fixed32, wire type 5 ou packed wire type 2).
- **Campo 3** (`gradient_angle`): `int32` (Varint, wire type 0).
- **Campo 4** (`base_mix`): `int32` (Varint, wire type 0).

### E. Mensagem `Favorites` (Campo 15 de `PreloadedUserSettings`)
- **Campo 1** (`favorite_channels`): `map<fixed64, FavoriteChannel>` (Repeated MapEntry, wire type 2):
  - Submensagem MapEntry:
    - **Campo 1** (`key`): `fixed64` (Wire type 1, 64-bit Snowflake do canal em little-endian).
    - **Campo 2** (`value`): Mensagem `FavoriteChannel` (Length-delimited, wire type 2).
- **Campo 2** (`muted`): `bool` (Varint, wire type 0).

### F. Mensagem `FavoriteChannel` (Valor do mapa `favorite_channels`)
- **Campo 1** (`nickname`): `string` opcional (Length-delimited, wire type 2).
- **Campo 2** (`parent_id`): `fixed64` opcional (Wire type 1).
- **Campo 3** (`position`): `uint32` (Varint, wire type 0). Posição de ordenação na lista de favoritos.
- **Campo 4** (`channel_id`): `fixed64` opcional (Wire type 1).
- **Campo 5** (`channel_type`): Wrapper `google.protobuf.UInt32Value` (Length-delimited, campo 1 varint uint32):
  - `1`: `DM` (Direct Message individual)
  - `3`: `GROUP_DM` (Grupo de Direct Message)
  - `0`: `GUILD_TEXT` (Canal de servidor - ignorado na extração de DMs fixadas)
  - `2`: `GUILD_VOICE` (Canal de voz de servidor - ignorado)

---

## 3. Implementação no `discord-rs`

A implementação em `src/protocol/user_settings_proto.rs` segue rigorosamente a política de zero dependências externas extras e zero dados fictícios:

1. **Decodificador Protobuf Wire**:
   - Implementado em ~250 linhas em Rust puro, tratando Varint (0), Fixed64 (1), Length-delimited (2) e Fixed32 (5).
   - Ignora com segurança campos desconhecidos sem falhar (forward-compatible).
2. **Decodificação Base64**:
   - Decodificador RFC 4648 compatível com Base64 padrão e URL-safe com/sem padding.
3. **Extração e Ordenação de DMs**:
   - Filtra os canais de `favorite_channels` mantendo apenas DMs (`channel_type == 1` ou `channel_type == 3` ou não especificado).
   - Ordena os canais de forma estável por `position` ascendente.
   - Preserva o cache local `~/.config/discord-rs/cache/pinned_dms.json` como fallback para bots ou inicialização offline.
4. **Tratamento de Falhas e Mutações**:
   - Se `user_settings_proto` estiver ausente ou corrompido, a lista local de DMs fixadas é preservada sem alterações.
   - Quando um evento `USER_SETTINGS_PROTO_UPDATE` válido é recebido, ele reordena as DMs fixadas no `AppState` chamando `sort_dm_channels()`.
5. **Aplicação do Tema**:
   - O tema decodificado é gravado em `AppState::account_theme`.
   - A seleção da paleta visual é feita por `DiscordPalette::for_theme(theme)` e `kit_theme::install_for_account_theme(cx, theme)`.
   - **Regra de Fallback para Tema Escuro**: Atualmente o cliente possui suporte de produção consolidado para a paleta escura (`DiscordPalette::dark()`). Temas não suportados (ex.: Light, Midnight ou presets de gradiente personalizados) caem automaticamente no tema Dark de referência, mantendo a integridade estética do app sem criar paletas fictícias.
