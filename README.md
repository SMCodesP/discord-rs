# Discord RS — Cliente Desktop Discord Nativo em Rust e GPUI

Um cliente desktop do Discord desenvolvido 100% em **Rust**, com renderização nativa de interface acelerada por hardware via **[GPUI](https://gpui.rs/)** (o framework gráfico de alto desempenho do Zed), sem qualquer uso de Electron, WebViews ou navegadores embutidos.

---

## 🎯 Destaques do Projeto

- **Performance Extrema & 60-120+ FPS**: Renderização acelerada por GPU direta (Vulkan no Linux, Metal no macOS, DirectX no Windows).
- **Fidelidade Visual e Comportamental**:
  - Tokens de design oficiais do Discord Dark Theme (paleta `#1e1f22`, `#2b2d31`, `#313338`, `#5865f2`, `#23a55a`).
  - Layout multi-colunas: Guild Rail (72px), Channel Sidebar (240px), Main Chat / Stage Area (Flex-1) e Member Sidebar (240px colapsável).
  - Stream de mensagens estilo **Cozy** com agrupamento de autor por janela de 5 minutos, avatares, embeds, reações e markdown rico.
  - Editor inferior interativo com envio otimista imediato e banner de resposta ativa.
  - Modal de autenticação nativo ("Conectar ao Discord") com suporte a Bot Token e User Token.
- **Dual Transport (Modo Híbrido)**:
  - **Modo Demonstração (Offline)**: Provedor determinístico pré-carregado com servidores, canais, membros e conversas realistas prontas para uso imediato sem necessidade de credenciais.
  - **Modo Ao Vivo (Live Discord)**: Conexão real com a API REST v10 e WebSocket Gateway (`wss://gateway.discord.gg/?v=10&encoding=json`) com loop de Heartbeat dinâmico, handshake Identify, e recepção em tempo real de mensagens e reações.
- **Armazenamento Seguro de Credenciais**:
  - Tokens persistidos em `~/.config/discord-rs/auth.json` com permissões Unix `0600` (somente leitura/escrita pelo usuário).
  - Suporte automático à variável de ambiente `DISCORD_TOKEN`.
- **Internacionalização**: Interface 100% em Português do Brasil (`pt-BR`) com arquitetura modular preparada para outros idiomas.

---

## 🏗️ Arquitetura do Sistema

```
discord-rs/
├── src/
│   ├── app/                 # App Shell, layout multi-colunas, modais e gerenciamento de estado
│   │   ├── guild_rail.rs        # Barra lateral esquerda de servidores (72px)
│   │   ├── channel_sidebar.rs   # Barra de canais e painel do usuário (240px)
│   │   ├── header_bar.rs        # Cabeçalho superior com tópico, busca e login (48px)
│   │   ├── member_sidebar.rs    # Lista de membros da guilda (240px colapsável)
│   │   ├── voice_call_panel.rs  # Painel de chamada de voz e WebRTC simulado
│   │   ├── user_settings.rs     # Modal de configurações do usuário
│   │   ├── root_view.rs         # Composição do shell principal e modais
│   │   └── state.rs             # Modelo reativo AppState observado pelo GPUI
│   ├── chat/                # Mecanismo de renderização do chat
│   │   ├── chat_view.rs         # View reativa do chat integrando histórico e editor
│   │   ├── message_list.rs      # Stream Cozy, agrupamento de 5min, hover bar
│   │   ├── markdown.rs          # Parser de markdown (spoilers, inline code, negrito, etc.)
│   │   ├── reactions.rs         # Badges de reação com contadores dinâmicos
│   │   └── editor.rs            # Campo de digitação, anexos e indicador de digitação
│   ├── components/          # Componentes visuais atômicos em GPUI
│   │   ├── avatar.rs            # Avatares com badges de presença (Online, Idle, DND, Offline)
│   │   ├── badge.rs             # Pílula de mensagens não lidas e badge de menção
│   │   ├── banner.rs            # Banners informativos e de erro (Info, Success, Warning, Danger)
│   │   ├── button.rs            # Botões Blurple, Gray, Danger, Ghost e IconButton
│   │   ├── channel_item.rs      # Itens de canal de texto e voz com hover e seleção
│   │   ├── guild_item.rs        # Ícones do rail com transição de círculo para squircle
│   │   ├── member_item.rs       # Linhas de membro com cargo e status de atividade
│   │   ├── modal.rs             # Estrutura base para diálogos modais
│   │   ├── text_input.rs        # Campo de texto interativo nativo GPUI com máscara
│   │   └── tooltip.rs           # Tooltips estilizados estilo Discord
│   ├── domain/              # Entidades e modelos de domínio
│   │   ├── channel.rs, guild.rs, message.rs, user.rs, voice.rs
│   │   └── mod.rs
│   ├── i18n/                # Dicionários de tradução (pt-BR e en-US)
│   ├── protocol/            # Comunicação, autenticação e Gateway
│   │   ├── auth.rs              # Gerenciador de tokens e persistência segura (0600)
│   │   ├── discord_api.rs       # Cliente REST v10 + WebSocket Gateway v10
│   │   ├── mock_provider.rs     # Provedor de dados determinístico offline
│   │   ├── client_manager.rs    # Despachador de transporte e ponte tokio broadcast
│   │   └── transport.rs         # Trait assíncrono DiscordTransport
│   ├── tokens/              # Tokens de design (cores, tipografia, espaçamento, movimento, ícones)
│   ├── lib.rs               # Exportação da biblioteca
│   └── main.rs              # Ponto de entrada, runtime Tokio e janela GPUI
├── docs/                    # Documentação técnica e relatórios de conformidade
│   ├── COMPATIBILITY_REPORT.md  # Diagnóstico de hardware, Vulkan e Wayland
│   ├── DESIGN_SPEC.md           # Especificação dimensional e de cores
│   ├── INTEGRATION_FEASIBILITY.md# Estudo de viabilidade de APIs e Gateway
│   ├── MOTION_CATALOG.md        # Catálogo de microinterações e transições
│   └── PARITY_MATRIX.md         # Matriz completa de paridade de recursos
├── Cargo.toml               # Dependências fixadas
└── README.md                # Este documento
```

---

## 🚀 Como Compilar e Executar

### Pré-requisitos (Linux / Arch / Manjaro)
- Rust 1.85+ (Edição 2024 recomendada).
- Bibliotecas do sistema para renderização Wayland/X11 e áudio:
  ```bash
  sudo pacman -S --needed base-devel vulkan-devel pipewire wayland
  ```

### 1. Compilação
```bash
# Validação rápida de tipos
cargo check --bin discord-rs

# Compilação do binário de desenvolvimento
cargo build --bin discord-rs
```

### 2. Execução
```bash
cargo run --bin discord-rs
```

### 3. Testes Unitários
```bash
RUST_MIN_STACK=67108864 cargo test --lib
```

---

## 🔑 Como Conectar com sua Conta do Discord

Você pode conectar o aplicativo de duas maneiras:

1. **Pela Interface Gráfica**:
   - Abra o aplicativo.
   - Clique no botão **🔑 Entrar no Discord** localizado no topo superior direito da janela.
   - Escolha o tipo de credencial:
     - **🤖 Bot Token**: Para bots criados no Discord Developer Portal.
     - **👤 Conta de Usuário**: Para testar com seu token pessoal.
   - Cole o token no campo correspondente e clique em **Conectar ao Discord**.
   - O cliente se conectará ao Gateway WebSocket v10, carregará seu perfil real e sincronizará seus servidores e canais.

2. **Via Variável de Ambiente**:
   ```bash
   export DISCORD_TOKEN="seu_token_aqui"
   cargo run --bin discord-rs
   ```

---

## 📄 Licença
Distribuído sob licença MIT ou Apache 2.0. Desenvolvido para fins educacionais e de pesquisa de renderização de alto desempenho em Rust e GPUI.
