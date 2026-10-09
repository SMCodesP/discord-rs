# ESCOPO TÉCNICO & ARQUITETURA: EMOJIS, FIGURINHAS (STICKERS), GIFS (TENOR), SOUNDBOARD E CACHE DE MÍDIA

Este documento estabelece o escopo de implementação, especificações de design, rotas REST do Discord v10, estratégias de particionamento e ciclo de vida de caching em disco para os subsistemas de Emojis, Stickers, GIFs e Soundboard no cliente **discord-rs**.

---

## 1. Visão Geral e Matriz de Paridade

O Discord Oficial Desktop possui um seletor unificado flutuante (Media Picker Popover) acoplado à barra de entrada do chat (`ChatView` / `ChatEditor`), com suporte a busca instantânea, navegação por teclado, pré-visualização ao passar o mouse (hover preview), e cache assíncrono particionado em disco.

| Recurso | Discord Oficial Desktop | discord-rs (Paridade) | Complexidade |
| :--- | :--- | :--- | :--- |
| **Abas do Seletor** | Emojis, Figurinhas (Stickers), GIFs, Soundboard | 4 abas nativas com alternância instantânea | Média |
| **Categorias de Emojis** | 8 categorias Unicode + Servidores + Recentes | 8 categorias canônicas + Servidores dinâmicos | Alta |
| **Frequentemente Usados** | Persistência local em disco com decaimento temporal | `~/.config/discord-rs/cache/media/recent_emojis.json` | Média |
| **Stickers (Figurinhas)** | PNG, APNG, Lottie (JSON) e WebP | Suporte a pacotes padrão (Wumpus) e de guildas | Alta |
| **Buscador de GIFs** | API v2 Tenor com Trending e Categorias | Integração Tenor v2 + Fallback local instantâneo | Média |
| **Soundboard** | Sons oficiais + Sons de servidores com atalhos | Visualizador de sons, botões de teste e envio | Média |
| **Cache de Mídia** | Sharding Content-Addressing (SHA-256) | `~/.config/discord-rs/cache/media/{00..ff}/{sha256}.bin` | Alta |
| **Lazy Loading & UI** | Zero congelamentos de thread da UI | Pipeline Single-Flight não-bloqueante via Tokio | Alta |

---

## 2. Arquitetura da Interface do Seletor Popup (Media Picker Popover)

### 2.1 Dimensões e Layout Visual
- **Largura**: `432px` fixos.
- **Altura**: `440px` fixos.
- **Posicionamento**: Ancorado no canto inferior direito do painel de chat, imediatamente acima da barra de botões do editor (`on_emoji`, `on_gif`, `on_sticker`), com sombra profunda (`shadow_xl`) e borda `#1e1f22`.
- **Paleta de Cores**:
  - Fundo principal: `#2b2d31` (`HEX_BG_CHANNELS`)
  - Barra de abas e cabeçalho: `#232428` (`HEX_BG_USER_BAR`)
  - Campo de busca: `#1e1f22` com texto `#dbdee1` e placeholder `#80848e`
  - Item em foco/hover: `#35373c` com transição suave
  - Rodapé de preview: `#232428` com ícone 32x32 e texto canônico `:nome:`

```
┌────────────────────────────────────────────────────────┐
│  [ 😊 Emojis ]  [ 🏷️ Stickers ]  [ 🎬 GIFs ]  [ 🔊 Sons ]   │ (Abas Superiores)
├────────────────────────────────────────────────────────┤
│  🔍 Buscar emojis, stickers, GIFs...              [X]  │ (Barra de Busca)
├──────┬─────────────────────────────────────────────────┤
│ 🕒   │  FREQUENTEMENTE USADOS                          │
│ 😀   │  [👍] [❤️] [🔥] [🎉] [🦀] [🚀] [👀] [😂]         │
│ 🐱   ├─────────────────────────────────────────────────┤
│ 🍔   │  PESSOAS & EXPRESSÕES                           │
│ ⚽   │  [😀] [😃] [😄] [😁] [😆] [😅] [😂] [🤣] [😊] [😇]│
│ ✈️   │  [🙂] [🙃] [😉] [😌] [😍] [🥰] [😘] [😗] [😙] [😚]│
│ 💡   ├─────────────────────────────────────────────────┤
│ 🔣   │  RUST BRASIL (CUSTOM EMOJIS)                    │
│ 🚩   │  [<:ferris:1>] [<:tokio:2>] [<:cargo:3>]        │
├──────┴─────────────────────────────────────────────────┤
│  🦀 :ferris: — Mascote oficial do Rust                 │ (Rodapé de Preview)
└────────────────────────────────────────────────────────┘
```

### 2.2 Abas do Seletor
1. **Aba Emojis**:
   - Barra lateral rápida com ícones das categorias para salto imediato (scroll jump).
   - Grade virtualizada de emojis 32x32px.
   - Suporte a tooltips nativos com nome em formato de shortcode `:emoji_name:`.
   - Clique insere o emoji no cursor do editor ou adiciona reação.

2. **Aba Figurinhas (Stickers)**:
   - Agrupamento por pacotes de stickers (ex: "Wumpus", "Discord Nitro", "Servidor Atual").
   - Exibição de cards de figurinha em formato quadrado de 64x64px ou 80x80px.
   - Tratamento de metadados: nome, descrição e tags de busca.

3. **Aba GIFs**:
   - Integração com a API Tenor do Google.
   - Exibição de carrossel de categorias trending ("Bom dia", "Obrigado", "Dança", "Anime", "Gatos").
   - Grade em 2 colunas com proporção adaptável e animação ao passar o mouse.
   - Clique envia diretamente a URL do GIF ou insere no texto.

4. **Aba Soundboard**:
   - Lista de sons padrão do Discord + sons da guilda atual.
   - Ícone/emoji do som, título e botão de preview de áudio (play instantâneo).
   - Envio de áudio para o canal de voz ativo ou envio de card de som no chat.

---

## 3. Mapeamento de Rotas da API REST do Discord v10

| Método | Rota Endpoint | Descrição | Resposta / Payload |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v10/guilds/{guild_id}/emojis` | Lista todos os emojis customizados do servidor | `Vec<DiscordEmoji>` |
| `GET` | `/api/v10/guilds/{guild_id}/emojis/{emoji_id}` | Obtém metadados de um emoji específico | `DiscordEmoji` |
| `GET` | `/api/v10/guilds/{guild_id}/stickers` | Lista stickers customizados da guilda | `Vec<DiscordSticker>` |
| `GET` | `/api/v10/sticker-packs` | Lista pacotes padrão de figurinhas do Discord Nitro | `{ "sticker_packs": Vec<StickerPack> }` |
| `GET` | `/api/v10/soundboard-default-sounds` | Sons oficiais padrão do Discord | `Vec<SoundboardSound>` |
| `GET` | `/api/v10/guilds/{guild_id}/soundboard-sounds` | Sons personalizados da Soundboard da guilda | `Vec<SoundboardSound>` |

### 3.1 Modelo de Dados dos Emojis (`DiscordEmoji`)
```json
{
  "id": "100200300400500600",
  "name": "ferris_happy",
  "roles": [],
  "user": {
    "id": "123456789",
    "username": "ferris_dev"
  },
  "require_colons": true,
  "managed": false,
  "animated": false,
  "available": true
}
```

### 3.2 Modelo de Dados dos Stickers (`DiscordSticker`)
```json
{
  "id": "749054660769218631",
  "pack_id": "847202553061146644",
  "name": "Wumpus Celebrate",
  "description": "Wumpus celebrating with confetti",
  "tags": "celebrate, party, yay, confetti",
  "asset": "",
  "type": 1,
  "format_type": 1,
  "available": true,
  "guild_id": null
}
```
*Tipos de Formato (`format_type`)*:
- `1`: PNG raster
- `2`: APNG animado
- `3`: Lottie JSON vetorial
- `4`: GIF raster

### 3.3 Modelo de Dados da Soundboard (`SoundboardSound`)
```json
{
  "sound_id": "1",
  "name": "Quack",
  "volume": 0.8,
  "emoji_id": null,
  "emoji_name": "🦆",
  "guild_id": null,
  "available": true
}
```

---

## 4. Integração da API Tenor v2 para GIFs

O Discord consome a API do Tenor v2 (Google Cloud Tenor API) através dos endpoints canônicos:
1. **Trending GIFs**:
   `GET https://tenor.googleapis.com/v2/featured?key={TENOR_KEY}&client_key=discord_rs&limit=24&media_filter=nanogif,tinygif,gif`
2. **Search GIFs**:
   `GET https://tenor.googleapis.com/v2/search?q={query}&key={TENOR_KEY}&client_key=discord_rs&limit=24&media_filter=nanogif,tinygif,gif`
3. **Categorias de GIFs (Categories)**:
   `GET https://tenor.googleapis.com/v2/categories?key={TENOR_KEY}&client_key=discord_rs`

### Resolução e Otimização para Desktop:
- **Preview em Grade**: Consome formato `tinygif` ou `nanogif` (< 150KB por card) para evitar gargalos de memória e lag no scroll.
- **Envio no Chat**: Envia a URL do formato padrão em GIF ou WebP/MP4 com embed automático.
- **Fallback Offline**: Biblioteca com os 16 GIFs virais mais frequentes do Discord embutida nativamente para demonstração e modo offline.

---

## 5. Arquitetura de Cache Local de Mídia (`~/.config/discord-rs/cache/media/`)

Para garantir que o carregamento de milhares de emojis e stickers nunca cause congelamento da interface (`0ms UI frame drops`), implementa-se uma camada de armazenamento particionada por endereçamento de conteúdo (CAS SHA-256):

### 5.1 Estrutura de Diretórios
```
~/.config/discord-rs/cache/media/
├── 00/
│   ├── 00a1b2c3d4e5f6...bin        <- Binário da mídia (imagem, áudio, lottie)
│   └── 00a1b2c3d4e5f6...meta.json  <- Metadados (url de origem, content-type, etag, expiração)
├── 01/
├── ...
├── ff/
├── recent_emojis.json              <- Lista de emojis frequentemente usados (persistência contínua)
└── tenor_cache.json                <- Cache de consultas de busca do Tenor com TTL de 15 minutos
```

### 5.2 Fluxo de Caching Não-Bloqueante (Single-Flight)
1. **Consulta na Memória (L1 Cache)**: Um `RwLock<HashMap<String, Vec<u8>>>` guarda em RAM as mídias mais recentes. Acesso em `< 1ms`.
2. **Consulta em Disco (L2 Cache)**: Se ausente na RAM, calcula o SHA-256 da URL canônica e lê o arquivo `.bin` do shard correspondente (`media/{hash[0..2]}/{hash}.bin`).
3. **Download em Background (Single-Flight)**: Se a mídia ainda não existir, o job é despachado para uma task assíncrona do Tokio. Múltiplas tentativas simultâneas para a mesma URL são agrupadas (`single-flight coalesce`), evitando downloads duplicados.
4. **Notificação Reativa**: Assim que o download é concluído e o arquivo salvo atomicamente via `.tmp_*` + rename, a view do GPUI é notificada para renderizar a textura sem travar a thread de layout.

### 5.3 Persistência dos "Frequentemente Usados" (`recent_emojis.json`)
Cada uso de emoji (seja no chat ou em reações) dispara o método `record_emoji_use(emoji, is_custom, id)`.
- Algoritmo de Pontuação de Frequência:
  $$\text{Score} = \text{Contador de Uso} \times e^{-\lambda (t_{agora} - t_{uso})}$$
- Permite que emojis muito usados permaneçam no topo, enquanto emojis esporádicos decaiam gradualmente, exatamente como no Discord oficial.

---

## 6. Próximos Passos de Implementação
1. Módulo [`src/protocol/media_cache.rs`](file:///home/smcodes/personal/discord-rs/src/protocol/media_cache.rs):
   - Estrutura `MediaCacheManager`, sharding SHA-256, gerenciamento de recentes e cliente Tenor.
   - Modelos de dados para Emojis, Stickers, StickerPacks e Soundboard.
2. Módulo [`src/chat/media_picker.rs`](file:///home/smcodes/personal/discord-rs/src/chat/media_picker.rs):
   - Componente visual GPUI `MediaPicker` com navegação por abas, busca reativa, e grades de renderização.
3. Conexão na [`ChatView`](file:///home/smcodes/personal/discord-rs/src/chat/chat_view.rs) e testes de verificação.
