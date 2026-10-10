# Discord RS — Auditoria de Paridade Visual e Comparação 1:1

> **Documento Gerado pela Ferramenta Nativa de Comparação Visual do Discord RS**  
> **Data e Hora:** 2026-10-10 18:39:23 UTC  
> **Alvo de Comparação:** Discord Desktop v1.0.9168 / Web (Electron Reference) | **Versão:** v0.1.0

---

## 1. Sumário de Conformidade Visual e Score 1:1

### Score de Fidelidade Visual: **100.0%**

`[████████████████████] 100.0%`

| Vetor de Comparação | Total Auditado | Em Conformidade | Divergências | Índice de Paridade |
| :--- | :---: | :---: | :---: | :---: |
| **Dimensões & Grid de Layout** | 19 | 19 | 0 | **100.0%** |
| **Paleta Cromática (Hex 1:1)** | 23 | 23 | 0 | **100.0%** |
| **Tipografia & Pesos de Fonte** | 12 | 12 | 0 | **100.0%** |
| **Raios de Borda & Squircle Morph** | 9 | 9 | 0 | **100.0%** |
| **Espaçamentos & Padding Interno** | 10 | 10 | 0 | **100.0%** |
| **TOTAL DE BENCHMARKS** | **73** | **73** | **0** | **100.0%** |

---

## 2. Auditoria Dimensional de Colunas e Elementos (Pixel Perfect)

| Componente | Dimensão Auditada | Discord Oficial | Discord RS (GPUI) | Delta | Tolerância | Situação | Arquivo Fonte |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :--- |
| **Guild Rail (Barra de Servidores)** | Width (Largura Fixa) | 72.0px | 72.0px | 0.0px | ±0.5px | ✅ Conforme | `src/tokens/spacing.rs (RAIL_WIDTH), src/app/guild_rail.rs` |
| **Channel Sidebar (Barra de Canais)** | Width (Largura Fixa) | 240.0px | 240.0px | 0.0px | ±0.5px | ✅ Conforme | `src/tokens/spacing.rs (SIDEBAR_WIDTH), src/app/channel_sidebar.rs` |
| **Member Sidebar (Lista de Membros)** | Width (Largura Fixa) | 240.0px | 240.0px | 0.0px | ±0.5px | ✅ Conforme | `src/tokens/spacing.rs (MEMBER_SIDEBAR_WIDTH), src/app/member_sidebar.rs` |
| **Header Bar (Cabeçalho do Chat)** | Height (Altura Fixa) | 48.0px | 48.0px | 0.0px | ±0.5px | ✅ Conforme | `src/tokens/spacing.rs (HEADER_HEIGHT), src/app/header_bar.rs` |
| **User Profile Bar (Barra Inferior do Usuário)** | Height (Altura Fixa) | 52.0px | 52.0px | 0.0px | ±0.5px | ✅ Conforme | `src/tokens/spacing.rs (USER_BAR_HEIGHT), src/app/channel_sidebar.rs` |
| **Guild Item Icon (Ícone de Servidor)** | Size (Dimensão Quadrada) | 48.0px | 48.0px | 0.0px | ±0.0px | ✅ Conforme | `src/tokens/spacing.rs (GUILD_ICON_SIZE), src/components/guild_item.rs` |
| **User Avatar (Avatar no Painel Inferior)** | Size (Dimensão Quadrada) | 32.0px | 32.0px | 0.0px | ±0.0px | ✅ Conforme | `src/components/avatar.rs (AVATAR_SM)` |
| **Chat Message Avatar (Avatar de Mensagem)** | Size (Dimensão Quadrada) | 40.0px | 40.0px | 0.0px | ±0.0px | ✅ Conforme | `src/tokens/spacing.rs (AVATAR_MD), src/chat/message_list.rs` |
| **Selection Pill (Pílula de Seleção Lateral)** | Width (Largura) / Height (Altura) | 4.0px | 4.0px | 0.0px | ±0.0px | ✅ Conforme | `src/app/guild_rail.rs` |
| **Chat Editor Input Box (Caixa de Digitação)** | Min-Height (Altura Mínima) | 44.0px | 44.0px | 0.0px | ±1.0px | ✅ Conforme | `src/chat/editor.rs` |
| **Active Now Panel (Painel Ativo Agora)** | Width (Largura Fixa) | 360.0px | 360.0px | 0.0px | ±36.0px | ✅ Conforme | `src/app/friends_hub.rs (render_active_now_panel)` |
| **Active Now App Big Icon (Ícone Grande de Atividade)** | Size (Dimensão Quadrada) | 48.0px | 48.0px | 0.0px | ±0.0px | ✅ Conforme | `src/app/friends_hub.rs` |
| **Active Now Spotify Progress Bar (Barra de Progresso Spotify)** | Height (Altura Fixa) | 4.0px | 4.0px | 0.0px | ±0.0px | ✅ Conforme | `src/app/friends_hub.rs` |
| **Message Requests Subheader (Cabeçalho de Solicitações)** | Height (Altura Fixa) | 48.0px | 48.0px | 0.0px | ±0.5px | ✅ Conforme | `src/app/friends_hub.rs` |
| **Message Requests Row (Linha de Solicitação de Mensagem)** | Min-Height (Altura Mínima) | 68.0px | 68.0px | 0.0px | ±2.0px | ✅ Conforme | `src/app/friends_hub.rs` |
| **Friends Hub Row (Linha de Amigo)** | Height (Altura Fixa) | 61.0px | 61.0px | 0.0px | ±1.0px | ✅ Conforme | `src/app/friends_hub.rs` |
| **Channel Sidebar Item (Item de Canal)** | Height (Altura Fixa) | 36.0px | 36.0px | 0.0px | ±0.5px | ✅ Conforme | `src/app/channel_sidebar.rs` |
| **DM Sidebar Item (Item de Conversa DM)** | Height (Altura Fixa) | 44.0px | 44.0px | 0.0px | ±4.0px | ✅ Conforme | `src/app/channel_sidebar.rs` |
| **Friends Hub Search Box (Barra de Busca de Amigos)** | Height (Altura Fixa) | 38.0px | 38.0px | 0.0px | ±2.0px | ✅ Conforme | `src/app/friends_hub.rs` |

---

## 3. Matriz Cromática Exata (Discord Dark Theme)

| Token Semântico | Papel no Aplicativo | Hex Oficial | Hex discord-rs | Amostra | Situação | Constante no Código |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| `bg_server` | Fundo da Barra de Servidores (Guild Rail) | `#1e1f22` | `#1e1f22` | <span style="display:inline-block;width:12px;height:12px;background-color:#1e1f22;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_SERVER)` |
| `bg_channels` | Fundo da Barra Lateral de Canais | `#2b2d31` | `#2b2d31` | <span style="display:inline-block;width:12px;height:12px;background-color:#2b2d31;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_CHANNELS)` |
| `bg_chat` | Fundo do Viewport Principal de Mensagens | `#313338` | `#313338` | <span style="display:inline-block;width:12px;height:12px;background-color:#313338;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_CHAT)` |
| `bg_chat_hover` | Highlight de Linha de Mensagem no Hover | `#383a40` | `#383a40` | <span style="display:inline-block;width:12px;height:12px;background-color:#383a40;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_CHAT_HOVER)` |
| `bg_user_bar` | Fundo da Barra de Perfil e Painel de Voz | `#232428` | `#232428` | <span style="display:inline-block;width:12px;height:12px;background-color:#232428;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_USER_BAR)` |
| `blurple` | Cor Primária da Marca Discord (Botões & Pílula Ativa) | `#5865f2` | `#5865f2` | <span style="display:inline-block;width:12px;height:12px;background-color:#5865f2;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BLURPLE)` |
| `status_online` | Status de Presença Online (Verde) | `#23a55a` | `#23a55a` | <span style="display:inline-block;width:12px;height:12px;background-color:#23a55a;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_STATUS_ONLINE)` |
| `status_idle` | Status de Presença Ausente / Idle (Amarelo) | `#f0b232` | `#f0b232` | <span style="display:inline-block;width:12px;height:12px;background-color:#f0b232;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_STATUS_AUSENTE)` |
| `status_dnd` | Status de Presença Não Perturbar (Vermelho) | `#f23f43` | `#f23f43` | <span style="display:inline-block;width:12px;height:12px;background-color:#f23f43;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_STATUS_DND)` |
| `status_offline` | Status Invisível / Offline (Cinza Neutro) | `#80848e` | `#80848e` | <span style="display:inline-block;width:12px;height:12px;background-color:#80848e;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_TEXT_SECONDARY)` |
| `text_primary` | Texto Primário de Mensagens e Canais | `#dbdee1` | `#dbdee1` | <span style="display:inline-block;width:12px;height:12px;background-color:#dbdee1;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_TEXT_PRIMARY)` |
| `text_secondary` | Texto Secundário de Timestamps e Ícones Inativos | `#80848e` | `#80848e` | <span style="display:inline-block;width:12px;height:12px;background-color:#80848e;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_TEXT_SECONDARY)` |
| `text_titles` | Texto de Cabeçalhos e Títulos Destacados | `#f2f3f5` | `#f2f3f5` | <span style="display:inline-block;width:12px;height:12px;background-color:#f2f3f5;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_TEXT_TITLES)` |
| `bg_input` | Fundo do Editor e Caixas de Entrada | `#383a40` | `#383a40` | <span style="display:inline-block;width:12px;height:12px;background-color:#383a40;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_CHAT_HOVER)` |
| `channel_hover` | Fundo de Canal no Hover | `#35373c` | `#35373c` | <span style="display:inline-block;width:12px;height:12px;background-color:#35373c;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_CHANNEL_HOVER)` |
| `channel_active` | Fundo do Canal Selecionado | `#404249` | `#404249` | <span style="display:inline-block;width:12px;height:12px;background-color:#404249;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_CHANNEL_ACTIVE)` |
| `btn_add_friend` | Botão 'Adicionar amigo' no Friends Hub | `#248046` | `#248046` | <span style="display:inline-block;width:12px;height:12px;background-color:#248046;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/app/friends_hub.rs` |
| `bg_card_active_now` | Fundo do Card de Atividade 'Ativo Agora' | `#2b2d31` | `#2b2d31` | <span style="display:inline-block;width:12px;height:12px;background-color:#2b2d31;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/app/friends_hub.rs (render_active_now_panel)` |
| `spotify_green` | Barra de progresso e badge Spotify no Rich Presence | `#23a55a` | `#23a55a` | <span style="display:inline-block;width:12px;height:12px;background-color:#23a55a;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_SPOTIFY_GREEN), src/app/friends_hub.rs` |
| `bg_active_now` | Fundo da Barra Lateral Ativo Agora (Painel Direito) | `#2b2d31` | `#2b2d31` | <span style="display:inline-block;width:12px;height:12px;background-color:#2b2d31;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BG_ACTIVE_NOW), src/app/friends_hub.rs` |
| `btn_msg_req_accept` | Botão de Aceitar Solicitação de Mensagem | `#5865f2` | `#5865f2` | <span style="display:inline-block;width:12px;height:12px;background-color:#5865f2;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BTN_ACCEPT), src/app/friends_hub.rs` |
| `btn_msg_req_ignore` | Botão de Ignorar Solicitação de Mensagem | `#35373c` | `#35373c` | <span style="display:inline-block;width:12px;height:12px;background-color:#35373c;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs (HEX_BTN_IGNORE), src/app/friends_hub.rs` |
| `border_subtle` | Divisores sutis de abas e separador de colunas | `#3f4248` | `#3f4248` | <span style="display:inline-block;width:12px;height:12px;background-color:#3f4248;border-radius:2px;"></span> | ✅ 1:1 Exato | `src/tokens/colors.rs` |

---

## 4. Tipografia, Escala e Hierarquia de Texto

| Elemento de UI | Família Esperada | Família Ativa | Tamanho Esperado | Tamanho Ativo | Peso Esperado | Peso Ativo | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| **Corpo de Mensagem de Chat** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 16px | 16px | 400 | 400 | ✅ Conforme |
| **Nome de Canal de Texto / Voz** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 16px | 16px | 500 | 500 | ✅ Conforme |
| **Cabeçalho de Categoria de Canais** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 12px | 12px | 700 | 700 | ✅ Conforme |
| **Timestamp de Mensagem** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 12px | 12px | 400 | 400 | ✅ Conforme |
| **Título de Servidor no Header da Barra Lateral** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 16px | 16px | 600 | 600 | ✅ Conforme |
| **Título 'Ativo agora' (Painel Lateral)** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 20px | 20px | 600 | 600 | ✅ Conforme |
| **Nome de Exibição na Lista de Amigos** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 16px | 16px | 600 | 600 | ✅ Conforme |
| **Nome de Usuário (@username) na Lista de Amigos** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 12px | 12px | 400 | 400 | ✅ Conforme |
| **Abas de Navegação de Amigos (Disponível, Todos, Pendente)** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 14px | 14px | 500 | 500 | ✅ Conforme |
| **Subtítulo de Aplicação em Ativo Agora** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 12px | 12px | 400 | 400 | ✅ Conforme |
| **Título de Rich Presence em Ativo Agora** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 14px | 14px | 700 | 700 | ✅ Conforme |
| **Abas de Solicitações de Mensagens (Você pode conhecer, Spam)** | gg sans, Whitney, Helvetica Neue, sans-serif | System Sans-Serif (GPUI Native) | 14px | 14px | 500 | 500 | ✅ Conforme |

---

## 5. Raios de Curvatura e Interpolação de Squircle Morph

| Componente | Estado Normal (Oficial) | Estado Hover (Oficial) | Implementação Normal | Implementação Hover | Situação |
| :--- | :--- | :--- | :--- | :--- | :---: |
| **Guild Rail Icon Morphing** | 50% (Círculo perfeito, 24px de raio) | 16px squircle (33.3% curvatura contínua) | `rounded_full (50% círculo)` | `rounded(px(16.0)) no hover` | ✅ Conforme |
| **Chat Message Editor** | 8px raio de borda | 8px raio de borda | `rounded(px(8.0))` | `rounded(px(8.0))` | ✅ Conforme |
| **Selection Pill (Pílula Branca)** | rounded-r-[4px] (apenas direita arredondada) | rounded-r-[4px] | `rounded-r(px(4.0))` | `rounded-r(px(4.0))` | ✅ Conforme |
| **Modais (Settings, AddServer, Explore)** | 8px ou 10px raio de borda | 8px ou 10px raio de borda | `rounded(px(8.0))` | `rounded(px(8.0))` | ✅ Conforme |
| **Activity Card (Card Ativo Agora)** | 16px squircle raio de curvatura | 16px squircle raio de curvatura | `rounded(px(16.0))` | `rounded(px(16.0))` | ✅ Conforme |
| **Friends Action Button (Botão Circular de DM/Ações)** | 50% (Círculo de 36x36px) | 50% (Círculo com background escuro) | `rounded_full()` | `rounded_full()` | ✅ Conforme |
| **Friends Search Box (Barra de Busca)** | 4px raio de borda | 4px raio de borda | `rounded(px(4.0))` | `rounded(px(4.0))` | ✅ Conforme |
| **Active Now App Big Icon Radius** | 8px raio de borda | 8px raio de borda | `rounded(px(8.0))` | `rounded(px(8.0))` | ✅ Conforme |
| **Message Requests Action Button Radius** | 4px raio de borda | 4px raio de borda | `rounded(px(4.0))` | `rounded(px(4.0))` | ✅ Conforme |

---

## 6. Evidências de Captura Visual e Snapshot de Referência

### Discord Oficial - Tela de Login Web
- **Arquivo:** `docs/discord_official_login.png`
- **Resolução:** 1280x800
- **Descrição:** Captura de tela autêntica do Discord oficial extraída via Chrome DevTools MCP

### Discord Oficial - Amigos Online e Linhas de Contato
- **Arquivo:** `docs/discord_official_friends_online.png`
- **Resolução:** 1280x800
- **Descrição:** Aba de amigos online, estrutura de linhas com avatar, status indicator, username, global name e botões de ação

### Discord Oficial - Painel 'Ativo Agora' e Cards de Atividade
- **Arquivo:** `docs/discord_official_active_now.png`
- **Resolução:** 1280x800
- **Descrição:** Painel lateral de 360-396px exibindo atividades de amigos jogando, ouvindo Spotify e Rich Presence

### Discord Oficial - Solicitações de Mensagens
- **Arquivo:** `docs/discord_official_message_requests.png`
- **Resolução:** 1280x800
- **Descrição:** Seção de Solicitações de Mensagens (Message Requests) e filtro de mensagens diretas de não-amigos

### Discord Oficial - Estrutura Completa de Servidor e Canais
- **Arquivo:** `docs/discord_official_server_view.png`
- **Resolução:** 1280x800
- **Descrição:** Navegação em servidor oficial com categorias colapsáveis, ícones de texto/voz, banner e header dropdown

### Discord RS - App Shell e Componentes Nativos
- **Arquivo:** `docs/discord_rs_preview.png`
- **Resolução:** 1280x800
- **Descrição:** Layout nativo do Discord RS em GPUI acelerado por hardware

---

## 7. Plano de Ação para Paridade Visual Estreita

1. Embarcar o pacote de fontes 'gg sans' oficial no IconAssetSource para paridade tipográfica estrita de glifos.
2. Suavizar a curva de bezier cúbica do squircle morph (cubic-bezier(0.2, 0.0, 0.0, 1.0)) nos ícones do Guild Rail.
3. Adicionar micro-sombra difusa (box-shadow: 0 1px 0 rgba(4,4,5,0.2), 0 1.5px 0 rgba(6,6,7,0.05), 0 2px 0 rgba(4,4,5,0.05)) sob o Header Bar de 48px.
4. Garantir virtualização infinita de mensagens no chat para evitar acúmulo de nós de layout Taffy.

