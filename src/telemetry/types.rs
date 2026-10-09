use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// SLAs padrão de latência para a UI do Discord RS (em microssegundos)
pub mod sla {
    /// Meta de 120 FPS para montagem de telas (8.33 ms)
    pub const TARGET_MOUNT_LATENCY_US: u64 = 8_333;
    /// SLA máximo tolerável antes de drop perceptível de frame em 60 FPS (16.66 ms)
    pub const MAX_MOUNT_LATENCY_US: u64 = 16_666;
    /// SLA para resposta imediata ao clique de interação (8.33 ms)
    pub const INTERACTION_RESPONSE_SLA_US: u64 = 8_333;
    /// SLA máximo para despacho otimista de mensagem no chat (50.0 ms)
    pub const MESSAGE_DISPATCH_SLA_US: u64 = 50_000;
    /// SLA máximo para resposta de handshake do Gateway WebSocket (250.0 ms)
    pub const GATEWAY_HANDSHAKE_SLA_US: u64 = 250_000;
}

/// Identificação de tela ativa no aplicativo Discord
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum ScreenKind {
    Guild {
        id: String,
        name: String,
    },
    TextChannel {
        id: String,
        name: String,
        guild_id: Option<String>,
    },
    VoiceChannel {
        id: String,
        name: String,
        guild_id: Option<String>,
    },
    AnnouncementChannel {
        id: String,
        name: String,
        guild_id: Option<String>,
    },
    DirectMessages {
        channel_id: Option<String>,
        recipient_name: Option<String>,
    },
    FriendsHub,
    UserSettingsModal {
        tab: String,
    },
    AddServerModal,
    ExploreServersModal,
    LoginModal,
    FullScreenLogin,
}

impl ScreenKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Guild { .. } => "Guild View",
            Self::TextChannel { .. } => "Text Channel View",
            Self::VoiceChannel { .. } => "Voice Channel Stage",
            Self::AnnouncementChannel { .. } => "Announcement Channel View",
            Self::DirectMessages { .. } => "Direct Messages View",
            Self::FriendsHub => "Friends Hub View",
            Self::UserSettingsModal { .. } => "User Settings Modal",
            Self::AddServerModal => "Add Server Modal",
            Self::ExploreServersModal => "Explore Servers Modal",
            Self::LoginModal => "Login Modal",
            Self::FullScreenLogin => "Full-Screen Login",
        }
    }

    pub fn identifier(&self) -> String {
        match self {
            Self::Guild { id, .. } => format!("guild:{}", id),
            Self::TextChannel { id, .. } => format!("channel:text:{}", id),
            Self::VoiceChannel { id, .. } => format!("channel:voice:{}", id),
            Self::AnnouncementChannel { id, .. } => format!("channel:announcement:{}", id),
            Self::DirectMessages { channel_id, .. } => {
                format!("dm:{}", channel_id.as_deref().unwrap_or("inbox"))
            }
            Self::FriendsHub => "home:friends".into(),
            Self::UserSettingsModal { tab } => format!("modal:settings:{}", tab),
            Self::AddServerModal => "modal:add_server".into(),
            Self::ExploreServersModal => "modal:explore".into(),
            Self::LoginModal => "modal:login".into(),
            Self::FullScreenLogin => "screen:login_fullscreen".into(),
        }
    }
}

/// Registro detalhado de uma transição entre telas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenTransitionEvent {
    pub id: u64,
    pub from_screen: Option<ScreenKind>,
    pub to_screen: ScreenKind,
    pub started_at_us: u64,
    pub completed_at_us: u64,
    pub duration_us: u64,
    pub duration_ms: f64,
    pub is_sla_violated: bool,
    pub metadata: HashMap<String, String>,
}

/// Tipos de interações de usuário capturadas pelo InteractionTracker
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details")]
pub enum InteractionKind {
    ButtonClick {
        button_id: String,
        label: String,
    },
    ChannelSelect {
        channel_id: String,
        channel_name: String,
        kind: String,
    },
    GuildSelect {
        guild_id: String,
        guild_name: String,
    },
    MessageAction {
        action: String,
        message_id: String,
    },
    ReactionToggle {
        emoji: String,
        message_id: String,
        added: bool,
    },
    TabChange {
        tab_name: String,
    },
    MessageSend {
        channel_id: String,
        byte_len: usize,
        is_optimistic: bool,
    },
    ChatScroll {
        delta_y: f32,
        scroll_top: f32,
    },
    VoiceAction {
        action: String,
        channel_id: String,
    },
    ModalAction {
        action: String,
        modal_name: String,
    },
}

impl InteractionKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ButtonClick { .. } => "button_click",
            Self::ChannelSelect { .. } => "channel_select",
            Self::GuildSelect { .. } => "guild_select",
            Self::MessageAction { .. } => "message_action",
            Self::ReactionToggle { .. } => "reaction_toggle",
            Self::TabChange { .. } => "tab_change",
            Self::MessageSend { .. } => "message_send",
            Self::ChatScroll { .. } => "chat_scroll",
            Self::VoiceAction { .. } => "voice_action",
            Self::ModalAction { .. } => "modal_action",
        }
    }
}

/// Evento de interação capturado com métricas de tempo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionEvent {
    pub id: u64,
    pub kind: InteractionKind,
    pub timestamp_us: u64,
    pub duration_us: u64,
    pub is_sla_violated: bool,
}

/// Ponto de rastro de navegação (Breadcrumb)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breadcrumb {
    pub timestamp_us: u64,
    pub category: String,
    pub message: String,
    pub data: HashMap<String, String>,
}

/// Categorias canônicas de subsistemas do cliente Discord
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParityCategory {
    NavigationLayout,
    ChatMessaging,
    VoiceAudio,
    VideoScreen,
    SettingsConfig,
    FriendsDm,
    ServerManagement,
    NetworkProtocol,
    SystemWindow,
}

impl ParityCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NavigationLayout => "Navegação & Layout",
            Self::ChatMessaging => "Chat & Mensageria",
            Self::VoiceAudio => "Áudio & Voz",
            Self::VideoScreen => "Vídeo & Telas",
            Self::SettingsConfig => "Configurações de Usuário",
            Self::FriendsDm => "Amigos & Mensagens Diretas",
            Self::ServerManagement => "Gestão de Servidores",
            Self::NetworkProtocol => "Rede & Gateway WebSocket",
            Self::SystemWindow => "Janela & Integração com o SO",
        }
    }
}

/// Classificação do status de paridade de um recurso/componente
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParityStatus {
    /// 100% de paridade funcional e visual com o Discord oficial
    FullParity,
    /// Funcionalidade implementada, porém com divergências visuais, comportamentais ou de API
    Divergent,
    /// Recurso presente no Discord oficial mas ainda ausente no cliente nativo
    MissingFeature,
    /// Recurso implementado, porém excede o orçamento de latência / orçamento de frames
    PerformanceBottleneck,
}

impl ParityStatus {
    pub fn weight(&self) -> f64 {
        match self {
            Self::FullParity => 1.0,
            Self::Divergent => 0.5,
            Self::PerformanceBottleneck => 0.35,
            Self::MissingFeature => 0.0,
        }
    }

    pub fn badge_str(&self) -> &'static str {
        match self {
            Self::FullParity => "FullParity",
            Self::Divergent => "Divergent",
            Self::MissingFeature => "MissingFeature",
            Self::PerformanceBottleneck => "PerformanceBottleneck",
        }
    }
}

/// Nível de prioridade para a lista de correções de paridade
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum PriorityLevel {
    /// P0: Bloqueador
    P0_Blocker = 0,
    /// P1: Crítico
    P1_Critical = 1,
    /// P2: Maior
    P2_Major = 2,
    /// P3: Menor
    P3_Minor = 3,
    P0Blocker = 10,
    P1Critical = 11,
    P2Major = 12,
    P3Minor = 13,
}

impl PriorityLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::P0_Blocker | Self::P0Blocker => "P0 (Bloqueador)",
            Self::P1_Critical | Self::P1Critical => "P1 (Crítico)",
            Self::P2_Major | Self::P2Major => "P2 (Maior)",
            Self::P3_Minor | Self::P3Minor => "P3 (Menor)",
        }
    }
}

/// Item priorizado no plano de ação de correção de paridade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrioritizedCorrection {
    pub priority: PriorityLevel,
    pub component_id: String,
    pub title: String,
    pub description: String,
    pub target_file: String,
    pub mitigation_plan: String,
    pub parity_score_delta: f64,
}

/// Estrutura para serialização do relatório de auditoria e paridade em JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParityAuditReport {
    pub generated_at: DateTime<Utc>,
    pub app_version: String,
    pub platform: String,
    pub compositor: String,
    pub global_parity_score: f64,
    pub total_features_mapped: usize,
    pub counts: ParityCounts,
    pub latency_metrics: LatencyMetrics,
    pub throughput_metrics: ThroughputMetrics,
    pub component_audits: Vec<ComponentAuditEntry>,
    pub prioritized_corrections: Vec<PrioritizedCorrection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParityCounts {
    pub full_parity: usize,
    pub divergent: usize,
    pub missing_feature: usize,
    pub performance_bottleneck: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyMetrics {
    pub screen_mount_p50_us: u64,
    pub screen_mount_p95_us: u64,
    pub screen_mount_p99_us: u64,
    pub screen_mount_max_us: u64,
    pub average_interaction_response_us: u64,
    pub total_screen_transitions: u64,
    pub total_mount_sla_violations: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThroughputMetrics {
    pub total_interactions: u64,
    pub total_button_clicks: u64,
    pub total_channel_selects: u64,
    pub total_guild_selects: u64,
    pub total_messages_sent: u64,
    pub total_reactions_toggled: u64,
    pub total_scroll_events: u64,
    pub total_interaction_sla_violations: u64,
    pub dropped_telemetry_events: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentAuditEntry {
    pub id: String,
    pub name: String,
    pub category: ParityCategory,
    pub status: ParityStatus,
    pub expected_behavior: String,
    pub current_behavior: String,
    pub source_location: String,
    pub measured_latency_us: Option<u64>,
    pub notes: String,
}
