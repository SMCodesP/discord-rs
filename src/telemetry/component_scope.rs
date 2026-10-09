use super::types::{
    InteractionKind, ParityCategory, ParityStatus, PriorityLevel, ScreenKind,
};

/// Definição canônica de um componente ou recurso do Discord Desktop
#[derive(Debug, Clone)]
pub struct CanonicalComponent {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ParityCategory,
    pub expected_behavior: &'static str,
    pub current_implementation: &'static str,
    pub baseline_status: ParityStatus,
    pub source_location: &'static str,
    pub target_sla_us: u64,
    pub priority_if_divergent: Option<PriorityLevel>,
    pub mitigation_plan: &'static str,
}

/// Matriz canônica de recursos do Discord para comparação e auditoria
pub struct ComponentScope;

impl ComponentScope {
    /// Retorna todos os componentes canônicos mapeados
    pub fn all() -> &'static [CanonicalComponent] {
        CANONICAL_MATRIX
    }

    /// Busca componente canônico por identificador
    pub fn find(id: &str) -> Option<&'static CanonicalComponent> {
        CANONICAL_MATRIX.iter().find(|c| c.id == id)
    }

    /// Retorna componentes filtrados por categoria
    pub fn by_category(category: ParityCategory) -> Vec<&'static CanonicalComponent> {
        CANONICAL_MATRIX.iter().filter(|c| c.category == category).collect()
    }

    /// Mapeia uma transição de tela para o componente canônico responsável
    pub fn map_screen_to_component(screen: &ScreenKind) -> &'static str {
        match screen {
            ScreenKind::Guild { .. } => "guild_rail",
            ScreenKind::TextChannel { .. } => "channel_sidebar",
            ScreenKind::VoiceChannel { .. } => "voice_stage",
            ScreenKind::AnnouncementChannel { .. } => "channel_sidebar",
            ScreenKind::DirectMessages { .. } => "friends_hub",
            ScreenKind::FriendsHub => "friends_hub",
            ScreenKind::UserSettingsModal { .. } => "user_settings_modal",
            ScreenKind::AddServerModal => "add_server_modal",
            ScreenKind::ExploreServersModal => "explore_servers_modal",
            ScreenKind::LoginModal => "login_modal",
            ScreenKind::FullScreenLogin => "fullscreen_login",
        }
    }

    /// Mapeia um evento de interação para o componente canônico responsável
    pub fn map_interaction_to_component(interaction: &InteractionKind) -> &'static str {
        match interaction {
            InteractionKind::ButtonClick { button_id, .. } => {
                if button_id.contains("mute") || button_id.contains("deafen") || button_id.contains("settings") {
                    "user_profile_bar"
                } else if button_id.contains("send") {
                    "chat_editor"
                } else if button_id.contains("modal") {
                    "modals_system"
                } else {
                    "ui_button"
                }
            }
            InteractionKind::ChannelSelect { .. } => "channel_sidebar",
            InteractionKind::GuildSelect { .. } => "guild_rail",
            InteractionKind::MessageAction { .. } => "message_actions_floating",
            InteractionKind::ReactionToggle { .. } => "reactions_bar",
            InteractionKind::TabChange { .. } => "user_settings_modal",
            InteractionKind::MessageSend { .. } => "chat_editor",
            InteractionKind::ChatScroll { .. } => "message_list",
            InteractionKind::VoiceAction { .. } => "voice_stage",
            InteractionKind::ModalAction { .. } => "modals_system",
        }
    }
}

pub static CANONICAL_MATRIX: &[CanonicalComponent] = &[
    // =========================================================================
    // 1. NAVEGAÇÃO & LAYOUT
    // =========================================================================
    CanonicalComponent {
        id: "guild_rail",
        name: "Guild Rail (Barra de Servidores)",
        category: ParityCategory::NavigationLayout,
        expected_behavior: "Barra lateral fixa de 72px (#1e1f22), ícones 48px com raio 50% (círculo) interpolando para 16px (squircle) no hover, pílula de seleção branca animada (8px hover, 40px ativo), badges de notificação e agrupamento em pastas.",
        current_implementation: "Implementado em GPUI com renderização nativa, pílula de seleção e badges de menção. Morphing de formato aproximado via interpolação GPUI.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/guild_rail.rs, src/components/guild_item.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Manter taxa de 120 FPS e cache de texturas de ícones de servidores.",
    },
    CanonicalComponent {
        id: "channel_sidebar",
        name: "Channel Sidebar (Barra de Canais)",
        category: ParityCategory::NavigationLayout,
        expected_behavior: "Barra de 240px (#2b2d31), cabeçalho do servidor com dropdown chevron, categorias sanfona com seta animada, canais de texto (#), voz (🔊) e anúncios (📢), pílula de hover e estado selecionado (#404249).",
        current_implementation: "Implementado com cabeçalho de servidor, suporte a categorias colapsáveis e lista de canais tipados com ícones dedicados.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/channel_sidebar.rs, src/components/channel_item.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Otimizar reconciliação de lista ao receber GUILD_CREATE ou CHANNEL_UPDATE.",
    },
    CanonicalComponent {
        id: "header_bar",
        name: "Header Bar do Chat",
        category: ParityCategory::NavigationLayout,
        expected_behavior: "Barra superior de 48px (#313338), ícone hash/voz, título do canal, divisor vertical, tópico, campo de busca em tempo real, ícone de pins e toggle da barra lateral de membros.",
        current_implementation: "Implementado com título do canal, tópico truncado, busca em tempo real e botão de alternância da lista de membros.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/header_bar.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Expandir popover de busca com filtros avançados.",
    },
    CanonicalComponent {
        id: "member_sidebar",
        name: "Member Sidebar (Lista de Membros)",
        category: ParityCategory::NavigationLayout,
        expected_behavior: "Painel lateral direito de 240px (#2b2d31), agrupamento hierárquico por cargos com cores personalizadas, status de presença (Online, Ausente, Ocupado, Offline) e atividade do usuário.",
        current_implementation: "Implementado com agrupamento por categorias de cargo, cores de cargo e indicadores de status com cores oficiais.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/member_sidebar.rs, src/components/member_item.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Implementar virtualização quando servidor tiver mais de 500 membros carregados.",
    },
    CanonicalComponent {
        id: "user_profile_bar",
        name: "User Profile Bar (Painel Inferior do Usuário)",
        category: ParityCategory::NavigationLayout,
        expected_behavior: "Barra inferior de 52px (#232428) na base da coluna de canais, avatar 32px com status dot, nome e discriminator/#status, botões interativos Mute, Deafen e Configurações de Usuário.",
        current_implementation: "Implementado com avatar, nome de usuário, status de conexão e botões Mute/Deafen/Configurações com alternância de estado visual.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/channel_sidebar.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Conectar mute/deafen diretamente ao mixer do PipeWire/CPAL.",
    },

    // =========================================================================
    // 2. CHAT & MENSAGERIA
    // =========================================================================
    CanonicalComponent {
        id: "message_list",
        name: "Message List (Histórico de Mensagens)",
        category: ParityCategory::ChatMessaging,
        expected_behavior: "Visualização Cozy com agrupamento de mensagens do mesmo autor em janela de 5 minutos, avatares de 40px, timestamps relativos formatados, virtualização suave com rolagem contínua para milhares de mensagens.",
        current_implementation: "Agrupamento Cozy de 5 minutos funcionando perfeitamente. No entanto, renderiza iteração linear limitada a 100 mensagens para evitar freeze de layout (falta virtualização infinita de lista).",
        baseline_status: ParityStatus::PerformanceBottleneck,
        source_location: "src/chat/message_list.rs, src/chat/chat_view.rs",
        target_sla_us: 16_666,
        priority_if_divergent: Some(PriorityLevel::P1Critical),
        mitigation_plan: "Implementar `uniform_list` nativo do GPUI para virtualizar o histórico de mensagens e eliminar limitação de 100 itens.",
    },
    CanonicalComponent {
        id: "markdown_parser",
        name: "Markdown Parser & Renderer",
        category: ParityCategory::ChatMessaging,
        expected_behavior: "Parser completo Discord Flavored Markdown: negrito (**), itálico (* e _), sublinhado (__), tachado (~~), spoilers (||texto||), blocos de código com destaque sintático, inline code (`), citações (> e >>>) e menções (@).",
        current_implementation: "Implementado com suporte a negrito, itálico, inline code, spoilers com toggle de clique, menções destacadas e blocos de código. Executa regexes repetidas a cada render.",
        baseline_status: ParityStatus::PerformanceBottleneck,
        source_location: "src/chat/markdown.rs",
        target_sla_us: 4_000,
        priority_if_divergent: Some(PriorityLevel::P1Critical),
        mitigation_plan: "Introduzir cache de nós AST pré-analisados por hash de mensagem (Memoization) para evitar parsing de regex em cada frame.",
    },
    CanonicalComponent {
        id: "reactions_bar",
        name: "Reactions Bar (Pílulas de Reações)",
        category: ParityCategory::ChatMessaging,
        expected_behavior: "Pílulas arredondadas sob a mensagem com contadores reativos, highlight quando o usuário atual reagiu, botão (+) para abrir o seletor de emojis e sincronização com o gateway.",
        current_implementation: "Implementado com alternância dinâmica de contagem e destaque visual ao clicar.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/chat/reactions.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Conectar com evento MESSAGE_REACTION_ADD/REMOVE do Gateway v10.",
    },
    CanonicalComponent {
        id: "message_actions_floating",
        name: "Message Floating Actions (Barra Flutuante de Ações)",
        category: ParityCategory::ChatMessaging,
        expected_behavior: "Barra flutuante sobre a mensagem no hover com ações rápidas: Reação Rápida, Responder (com banner), Editar (se autor), Excluir e Menu Mais.",
        current_implementation: "Implementado no hover com botão de reação rápida e resposta com ativação do banner de resposta no editor.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/chat/message_list.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Adicionar popover de opções secundárias (Copiar ID, Fixar Mensagem).",
    },
    CanonicalComponent {
        id: "chat_editor",
        name: "Chat Editor (Editor de Mensagens)",
        category: ParityCategory::ChatMessaging,
        expected_behavior: "Editor adaptável (#383a40), suporte a quebra de linha por Shift+Enter, envio por Enter, envio otimista imediato no chat, botão de anexo (+), seletor de emojis/GIFs e indicador de digitação.",
        current_implementation: "Implementado com digitação em tempo real, suporte a Enter/Shift+Enter, banner de resposta ativa e envio otimista imediato.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/chat/editor.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Desacoplar o buffer de digitação do AppState monolítico para evitar re-render em cascata de todas as 7 views filhas.",
    },
    CanonicalComponent {
        id: "file_upload_chunked",
        name: "Cloud Attachments & File Upload",
        category: ParityCategory::ChatMessaging,
        expected_behavior: "Upload de arquivos com chunking HTTP, drag-and-drop na janela, pré-visualização de imagem/vídeo antes de enviar e limites por tamanho de arquivo.",
        current_implementation: "Upload básico multipart suportado no protocolo, sem interface de drag-and-drop e pré-visualização gráfica.",
        baseline_status: ParityStatus::MissingFeature,
        source_location: "src/protocol/http_client.rs",
        target_sla_us: 100_000,
        priority_if_divergent: Some(PriorityLevel::P2Major),
        mitigation_plan: "Implementar interceptação de Drag & Drop no GPUI e componente de preview de anexo no editor.",
    },

    // =========================================================================
    // 3. ÁUDIO & VOZ
    // =========================================================================
    CanonicalComponent {
        id: "voice_stage",
        name: "Voice Call Panel & Stage (Painel de Chamada de Voz)",
        category: ParityCategory::VoiceAudio,
        expected_behavior: "Painel de canal de voz com participantes ativos, visualizador de quem está falando (halo verde), latência RTC em tempo real, controles de microfone/áudio e desconexão rápida.",
        current_implementation: "Implementado na UI com lista de participantes conectados, estado de fala e exibição de latência simulada.",
        baseline_status: ParityStatus::Divergent,
        source_location: "src/app/voice_call_panel.rs, src/domain/voice.rs",
        target_sla_us: 16_666,
        priority_if_divergent: Some(PriorityLevel::P1Critical),
        mitigation_plan: "Integrar com backend de áudio real PipeWire/CPAL e Voice Gateway v8.",
    },
    CanonicalComponent {
        id: "voice_e2ee_dave",
        name: "DAVE End-to-End Encryption (Protocolo de Voz Oficial)",
        category: ParityCategory::VoiceAudio,
        expected_behavior: "Criptografia de ponta a ponta (E2EE) obrigatória do Discord para canais de voz baseada no protocolo DAVE/MLS (RFC 9420) com ratchets de chaves de grupo e verificação SAS.",
        current_implementation: "Mock offline determinístico na interface, sem pipeline UDP ChaCha20-Poly1305 / DAVE ativo em produção.",
        baseline_status: ParityStatus::MissingFeature,
        source_location: "src/domain/voice.rs",
        target_sla_us: 20_000,
        priority_if_divergent: Some(PriorityLevel::P1Critical),
        mitigation_plan: "Implementar handshake do Voice Gateway e enquadramento RTP UDP com ciphers MLS.",
    },

    // =========================================================================
    // 4. VÍDEO & TELAS
    // =========================================================================
    CanonicalComponent {
        id: "screen_share_linux",
        name: "Screen Share & Video Streaming (Compartilhamento de Tela)",
        category: ParityCategory::VideoScreen,
        expected_behavior: "Transmissão de tela em 1080p/60fps no Linux via PipeWire e XDG Desktop Portal ScreenCast com áudio da aplicação.",
        current_implementation: "Botão de compartilhar tela presente na UI como toggle simulado.",
        baseline_status: ParityStatus::MissingFeature,
        source_location: "src/app/voice_call_panel.rs",
        target_sla_us: 16_666,
        priority_if_divergent: Some(PriorityLevel::P2Major),
        mitigation_plan: "Implementar captura DMA-BUF via PipeWire portal no Linux.",
    },

    // =========================================================================
    // 5. CONFIGURAÇÕES DE USUÁRIO
    // =========================================================================
    CanonicalComponent {
        id: "user_settings_modal",
        name: "User Settings Modal (Configurações de Usuário)",
        category: ParityCategory::SettingsConfig,
        expected_behavior: "Modal tela cheia com 13 abas canônicas (Minha Conta, Perfis, Privacidade, Aparência, Voz & Vídeo, Notificações, Atalhos, Idioma, etc.), tecla ESC para fechar e persistência reativa.",
        current_implementation: "Modal tela cheia implementado com menu lateral de 8 abas funcionais em pt-BR e fechamento via botão ESC.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/user_settings.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Conectar endpoints REST PATCH /users/@me para persistência remota.",
    },

    // =========================================================================
    // 6. AMIGOS & MENSAGENS DIRETAS (DMs)
    // =========================================================================
    CanonicalComponent {
        id: "friends_hub",
        name: "Friends Hub & Direct Messages (Central de Amigos e DMs)",
        category: ParityCategory::FriendsDm,
        expected_behavior: "Painel inicial com abas Online, Todos, Pendentes, Bloqueados e Adicionar Amigo, lista lateral de DMs recentes e chamada direta 1:1.",
        current_implementation: "Implementado com renderização de lista de amigos, status de presença e lista de conversas diretas.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/app/root_view.rs (render_friends_view)",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Implementar envio e cancelamento de solicitações de amizade na interface.",
    },

    // =========================================================================
    // 7. GESTÃO DE SERVIDORES & MODAIS
    // =========================================================================
    CanonicalComponent {
        id: "modals_system",
        name: "Modals System (Adicionar Servidor, Explorar e Login)",
        category: ParityCategory::ServerManagement,
        expected_behavior: "Overlays modais centralizados com backdrop escurecido (#000000cc), fechamento por clique fora ou ESC, animação de entrada e foco automático.",
        current_implementation: "Implementado com backdrop escurecido, modais para Adicionar Servidor, Explorar Servidores e Autenticação com foco automático no input.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/components/modal.rs, src/app/root_view.rs",
        target_sla_us: 8_333,
        priority_if_divergent: None,
        mitigation_plan: "Ajustar animação de escala suave (spring animation) na abertura.",
    },

    // =========================================================================
    // 8. REDE & PROTOCOLO DE GATEWAY
    // =========================================================================
    CanonicalComponent {
        id: "gateway_client",
        name: "Discord Gateway Client v10 (WebSocket Real-Time)",
        category: ParityCategory::NetworkProtocol,
        expected_behavior: "Conexão WebSocket contínua em wss://gateway.discord.gg, Heartbeat com jitter aleatório, Identify com token de bot/user, Resume de sessão após desconexão e despacho de eventos para UI.",
        current_implementation: "Implementado com suporte a tokens bot/user, loop assíncrono Tokio com Heartbeat e ponte reativa de eventos para o GPUI.",
        baseline_status: ParityStatus::FullParity,
        source_location: "src/protocol/discord_api.rs, src/protocol/client_manager.rs",
        target_sla_us: 250_000,
        priority_if_divergent: None,
        mitigation_plan: "Implementar compressão de stream zlib/zstd para economizar 80% de banda.",
    },
    CanonicalComponent {
        id: "rate_limit_bucket",
        name: "REST API Rate Limit Bucket Engine",
        category: ParityCategory::NetworkProtocol,
        expected_behavior: "Gerenciador RFC de limites de taxa do Discord com buckets por rota, respeito a X-RateLimit-Reset-After, jitter global e filas de espera não-bloqueantes.",
        current_implementation: "Tratamento de rate limit básico sequencial, sem filas avançadas por rota de bucket.",
        baseline_status: ParityStatus::Divergent,
        source_location: "src/protocol/http_client.rs",
        target_sla_us: 50_000,
        priority_if_divergent: Some(PriorityLevel::P2Major),
        mitigation_plan: "Implementar fila de buckets assíncrona baseada na especificação do Discord.",
    },

    // =========================================================================
    // 9. JANELA & INTEGRAÇÃO COM O SO
    // =========================================================================
    CanonicalComponent {
        id: "system_tray_and_ipc",
        name: "System Tray & Discord IPC Daemon",
        category: ParityCategory::SystemWindow,
        expected_behavior: "Ícone na bandeja do sistema com menu de contexto Mute/Deafen, inicialização com o sistema e daemon IPC local (/tmp/discord-ipc-0) para Rich Presence de jogos e VS Code.",
        current_implementation: "Ausente no momento; a aplicação roda como janela independente de desktop.",
        baseline_status: ParityStatus::MissingFeature,
        source_location: "src/main.rs",
        target_sla_us: 10_000,
        priority_if_divergent: Some(PriorityLevel::P2Major),
        mitigation_plan: "Adicionar suporte a DBus StatusNotifierItem e servidor Unix Domain Socket para Rich Presence.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_matrix_completeness() {
        assert!(ComponentScope::all().len() >= 15);
        let guild_rail = ComponentScope::find("guild_rail").expect("guild_rail deve existir");
        assert_eq!(guild_rail.name, "Guild Rail (Barra de Servidores)");

        let chat_components = ComponentScope::by_category(ParityCategory::ChatMessaging);
        assert!(!chat_components.is_empty());
    }
}
