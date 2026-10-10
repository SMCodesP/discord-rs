use crate::domain::{
    Activity, Attachment, Author, Category, Channel, Embed, EmbedField, Guild, Message,
    MessageStickerItem, PresenceStatus, Reaction, Role, User,
};
use crate::protocol::media_cache::{DiscordEmoji, DiscordSticker, StickerFormatType};
use crate::protocol::transport::{DiscordTransport, Relationship, TransportEvent};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

struct MockState {
    current_user: User,
    guilds: Vec<Guild>,
    messages: HashMap<String, Vec<Message>>,
    connected: bool,
    next_message_id: u64,
}

/// Deterministic mock provider simulating Discord guilds, channels, messages, and gateway events.
///
/// Fully offline, suitable for UI development with GPUI and unit/integration testing.
#[derive(Clone)]
pub struct MockDiscordProvider {
    state: Arc<RwLock<MockState>>,
    event_sender: broadcast::Sender<TransportEvent>,
}

impl MockDiscordProvider {
    /// Creates a new mock provider pre-populated with "Rust Brasil" and "GPUI Hub".
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(128);

        let current_user = User {
            id: "user-self-001".to_string(),
            username: "rustacean_nexus".to_string(),
            discriminator: None,
            global_name: Some("Nexus".to_string()),
            avatar: None,
            bot: false,
            presence: PresenceStatus::Online,
            activity: Some(Activity::playing("discord-rs (GPUI)")),
            avatar_decoration_data: None,
        };

        // --- Guild 1: Rust Brasil ---
        let rust_br_id = "guild-rust-br";
        let cat_br_welcome_id = "cat-br-welcome";
        let cat_br_text_id = "cat-br-text";
        let cat_br_voice_id = "cat-br-voice";

        let br_categories = vec![
            Category::new(cat_br_welcome_id, "BOAS-VINDAS", 1),
            Category::new(cat_br_text_id, "CANAIS DE TEXTO", 2),
            Category::new(cat_br_voice_id, "CANAIS DE VOZ", 3),
        ];

        let chan_br_rules = Channel::announcement("chan-br-rules", "boas-vindas")
            .with_guild_id(rust_br_id)
            .with_category_id(cat_br_welcome_id)
            .with_topic("Regras e informações da comunidade Rust Brasil")
            .with_position(1);

        let chan_br_general = Channel::text("chan-br-general", "geral")
            .with_guild_id(rust_br_id)
            .with_category_id(cat_br_text_id)
            .with_topic("Discussões gerais sobre a linguagem Rust")
            .with_position(2);

        let chan_br_help = Channel::text("chan-br-help", "duvidas-iniciantes")
            .with_guild_id(rust_br_id)
            .with_category_id(cat_br_text_id)
            .with_topic("Espaço acolhedor para tirar dúvidas de quem está começando")
            .with_position(3);

        let chan_br_projects = Channel::text("chan-br-projects", "projetos")
            .with_guild_id(rust_br_id)
            .with_category_id(cat_br_text_id)
            .with_topic("Compartilhe seus projetos e bibliotecas open-source")
            .with_position(4);

        let chan_br_voice_gen = Channel::voice("chan-br-voice-gen", "Voz Geral")
            .with_guild_id(rust_br_id)
            .with_category_id(cat_br_voice_id)
            .with_position(5);

        let chan_br_voice_pair = Channel::voice("chan-br-voice-pair", "Pair Programming")
            .with_guild_id(rust_br_id)
            .with_category_id(cat_br_voice_id)
            .with_position(6);

        let br_roles = vec![
            Role::new("role-br-everyone", "@everyone").with_position(0),
            Role::new("role-br-mod", "Moderador")
                .with_color(0x2ECC71)
                .with_hoist(true)
                .with_position(1),
            Role::new("role-br-dev", "Rustacean")
                .with_color(0xF46623)
                .with_hoist(true)
                .with_position(2),
        ];

        let rust_brasil_guild = Guild::new(rust_br_id, "Rust Brasil", "user-owner-br")
            .with_categories(br_categories)
            .with_channels(vec![
                chan_br_rules,
                chan_br_general,
                chan_br_help,
                chan_br_projects,
                chan_br_voice_gen,
                chan_br_voice_pair,
            ])
            .with_roles(br_roles);
        let mut rust_brasil_guild = rust_brasil_guild;
        rust_brasil_guild.emojis = vec![
            DiscordEmoji::new_custom("1001", "ferris", false, Some(rust_br_id.to_string())),
            DiscordEmoji::new_custom("1002", "rustacean", false, Some(rust_br_id.to_string())),
            DiscordEmoji::new_custom("1003", "crab_party", true, Some(rust_br_id.to_string())),
            DiscordEmoji::new_custom("1004", "cargo_ship", false, Some(rust_br_id.to_string())),
            DiscordEmoji::new_custom("1005", "unsafe_code", false, Some(rust_br_id.to_string())),
        ];
        rust_brasil_guild.stickers = vec![
            DiscordSticker {
                id: "781291131828699156".into(),
                name: "Ferris Wave".into(),
                description: Some("Ferris dando tchauzinho".into()),
                tags: "ferris,wave,hi".into(),
                format_type: StickerFormatType::Png,
                available: true,
                guild_id: Some(rust_br_id.to_string()),
                pack_id: None,
            },
            DiscordSticker {
                id: "781291131828699157".into(),
                name: "Ferris Hype".into(),
                description: Some("Ferris comemorando".into()),
                tags: "ferris,hype,party".into(),
                format_type: StickerFormatType::Png,
                available: true,
                guild_id: Some(rust_br_id.to_string()),
                pack_id: None,
            },
        ];

        // --- Guild 2: GPUI Hub ---
        let gpui_hub_id = "guild-gpui-hub";
        let cat_gpui_welcome_id = "cat-gpui-welcome";
        let cat_gpui_dev_id = "cat-gpui-dev";
        let cat_gpui_voice_id = "cat-gpui-voice";

        let gpui_categories = vec![
            Category::new(cat_gpui_welcome_id, "WELCOME", 1),
            Category::new(cat_gpui_dev_id, "DEVELOPMENT", 2),
            Category::new(cat_gpui_voice_id, "AUDIO & HANGOUT", 3),
        ];

        let chan_gpui_announcements = Channel::announcement("chan-gpui-announcements", "announcements")
            .with_guild_id(gpui_hub_id)
            .with_category_id(cat_gpui_welcome_id)
            .with_topic("GPUI releases, breaking changes, and updates")
            .with_position(1);

        let chan_gpui_general = Channel::text("chan-gpui-general", "general")
            .with_guild_id(gpui_hub_id)
            .with_category_id(cat_gpui_dev_id)
            .with_topic("General GPUI framework discussions and questions")
            .with_position(2);

        let chan_gpui_showcase = Channel::text("chan-gpui-showcase", "showcase")
            .with_guild_id(gpui_hub_id)
            .with_category_id(cat_gpui_dev_id)
            .with_topic("Show off desktop apps built with GPUI")
            .with_position(3);

        let chan_gpui_gpu = Channel::text("chan-gpui-gpu", "gpu-rendering")
            .with_guild_id(gpui_hub_id)
            .with_category_id(cat_gpui_dev_id)
            .with_topic("Metal, Vulkan, and DirectX backend performance")
            .with_position(4);

        let chan_gpui_dev_voice = Channel::voice("chan-gpui-dev-voice", "Dev Chat 1")
            .with_guild_id(gpui_hub_id)
            .with_category_id(cat_gpui_voice_id)
            .with_position(5);

        let chan_gpui_lounge = Channel::voice("chan-gpui-lounge", "Lounge")
            .with_guild_id(gpui_hub_id)
            .with_category_id(cat_gpui_voice_id)
            .with_position(6);

        let gpui_roles = vec![
            Role::new("role-gpui-everyone", "@everyone").with_position(0),
            Role::new("role-gpui-core", "Zed / GPUI Core")
                .with_color(0x5865F2)
                .with_hoist(true)
                .with_position(1),
            Role::new("role-gpui-builder", "App Builder")
                .with_color(0x3BA55D)
                .with_hoist(true)
                .with_position(2),
        ];

        let gpui_hub_guild = Guild::new(gpui_hub_id, "GPUI Hub", "user-owner-gpui")
            .with_categories(gpui_categories)
            .with_channels(vec![
                chan_gpui_announcements,
                chan_gpui_general,
                chan_gpui_showcase,
                chan_gpui_gpu,
                chan_gpui_dev_voice,
                chan_gpui_lounge,
            ])
            .with_roles(gpui_roles);
        let mut gpui_hub_guild = gpui_hub_guild;
        gpui_hub_guild.emojis = vec![
            DiscordEmoji::new_custom("2001", "zed_editor", false, Some(gpui_hub_id.to_string())),
            DiscordEmoji::new_custom("2002", "gpu_accel", false, Some(gpui_hub_id.to_string())),
            DiscordEmoji::new_custom("2003", "pixel_perfect", false, Some(gpui_hub_id.to_string())),
            DiscordEmoji::new_custom("2004", "fps_144", true, Some(gpui_hub_id.to_string())),
        ];
        gpui_hub_guild.stickers = vec![
            DiscordSticker {
                id: "781291131828699158".into(),
                name: "GPUI Rocket".into(),
                description: Some("Foguete GPUI voando a 144fps".into()),
                tags: "gpui,fast,rocket".into(),
                format_type: StickerFormatType::Png,
                available: true,
                guild_id: Some(gpui_hub_id.to_string()),
                pack_id: None,
            },
        ];

        // --- Pre-populate message history ---
        let now = Utc::now();
        let mut messages = HashMap::new();

        // Messages for chan-br-general
        let author_ferran = Author {
            id: "user-br-1".to_string(),
            username: "ferran_rs".to_string(),
            global_name: Some("Ferran".to_string()),
            avatar: None,
            bot: false,
        };

        let author_ana = Author {
            id: "user-br-2".to_string(),
            username: "ana_code".to_string(),
            global_name: Some("Ana Paula".to_string()),
            avatar: None,
            bot: false,
        };

        let author_carlos = Author {
            id: "user-br-3".to_string(),
            username: "carlos_systems".to_string(),
            global_name: Some("Carlos".to_string()),
            avatar: None,
            bot: false,
        };

        let author_alice = Author {
            id: "user-br-4".to_string(),
            username: "alice_tokio".to_string(),
            global_name: Some("Alice".to_string()),
            avatar: None,
            bot: false,
        };

        let msg_br_1 = Message::new(
            "msg-br-101",
            "chan-br-general",
            author_ferran.clone(),
            "Olá pessoal! Sejam todos muito bem-vindos ao Discord RS, nosso cliente nativo construído 100% em Rust com GPUI! 🦀🚀",
        )
        .with_guild_id(rust_br_id)
        .with_timestamp(now - Duration::minutes(45))
        .with_reactions(vec![
            Reaction::new("🦀", 8, true),
            Reaction::new("❤️", 4, false),
            Reaction::new("🚀", 6, false),
        ]);

        let msg_br_2 = Message::new(
            "msg-br-102",
            "chan-br-general",
            author_ana,
            "A velocidade de renderização do GPUI usando aceleração por GPU é inacreditável! Frames rodando cravados em 120Hz no Wayland.",
        )
        .with_guild_id(rust_br_id)
        .with_timestamp(now - Duration::minutes(32))
        .with_reactions(vec![Reaction::new("🔥", 5, false)]);

        let msg_br_3 = Message::new(
            "msg-br-103",
            "chan-br-general",
            author_carlos,
            "Como está estruturada a camada de sincronização entre o Tokio async runtime e os elementos visuais do GPUI?",
        )
        .with_guild_id(rust_br_id)
        .with_timestamp(now - Duration::minutes(15));

        let msg_br_4 = Message::new(
            "msg-br-104",
            "chan-br-general",
            author_alice,
            "Usamos um bridge com `tokio::sync::broadcast` canalizando eventos do Gateway diretamente para models observáveis no GPUI. Zero lag no main thread!",
        )
        .with_guild_id(rust_br_id)
        .with_timestamp(now - Duration::minutes(8))
        .with_reactions(vec![Reaction::new("💡", 3, true), Reaction::new("👏", 2, false)]);

        messages.insert(
            "chan-br-general".to_string(),
            vec![msg_br_1, msg_br_2, msg_br_3, msg_br_4],
        );

        // Messages for chan-br-rules
        let author_mod = Author {
            id: "user-br-mod".to_string(),
            username: "rust_moderator".to_string(),
            global_name: Some("Moderação Rust BR".to_string()),
            avatar: None,
            bot: true,
        };
        let msg_rules = Message::new(
            "msg-br-001",
            "chan-br-rules",
            author_mod,
            "Bem-vindo à comunidade brasileira de Rust! Por favor, respeite o código de conduta e mantenha os debates técnicos e produtivos.",
        )
        .with_guild_id(rust_br_id)
        .with_timestamp(now - Duration::hours(24));
        messages.insert("chan-br-rules".to_string(), vec![msg_rules]);

        // Messages for chan-br-projects
        let msg_proj = Message::new(
            "msg-br-201",
            "chan-br-projects",
            Author::new("user-br-5", "marcos_rust"),
            "Compartilhando a arquitetura inicial do Discord RS: módulo `domain` com tipos puros e `protocol` com trait assíncrona desacoplada.",
        )
        .with_guild_id(rust_br_id)
        .with_timestamp(now - Duration::minutes(20))
        .with_attachments(vec![Attachment {
            id: "att-arch-1".to_string(),
            filename: "architecture_diagram.png".to_string(),
            size: 204850,
            url: "https://raw.githubusercontent.com/rust-lang/rust-artwork/master/logo/rust-logo-512x512.png".to_string(),
            proxy_url: None,
            content_type: Some("image/png".to_string()),
            width: Some(512),
            height: Some(512),
        }]);
        messages.insert("chan-br-projects".to_string(), vec![msg_proj]);

        // Messages for chan-gpui-general
        let author_nathan = Author {
            id: "user-gpui-1".to_string(),
            username: "nathan_gpui".to_string(),
            global_name: Some("Nathan".to_string()),
            avatar: None,
            bot: false,
        };
        let msg_gpui_1 = Message::new(
            "msg-gpui-101",
            "chan-gpui-general",
            author_nathan,
            "Welcome to the GPUI Hub! GPUI is a hybrid 2D/3D immediate-style rendering framework designed for responsive desktop productivity apps.",
        )
        .with_guild_id(gpui_hub_id)
        .with_timestamp(now - Duration::hours(2))
        .with_embeds(vec![Embed {
            title: Some("GPUI - High Performance UI Framework for Rust".to_string()),
            description: Some("Built by Zed Industries for lightning fast code editing.".to_string()),
            url: Some("https://zed.dev".to_string()),
            color: Some(0x5865F2),
            author: None,
            fields: vec![EmbedField {
                name: "Render Backend".to_string(),
                value: "Metal (macOS) / Vulkan (Linux/Windows)".to_string(),
                inline: true,
            }],
            footer: None,
            image_url: None,
            thumbnail_url: None,
            timestamp: None,
        }]);
        messages.insert("chan-gpui-general".to_string(), vec![msg_gpui_1]);

        // Seed messages for direct messages
        let msg_df1 = Message::new(
            "msg-df-1",
            "chan-dm-ferris",
            author_ferran.clone(),
            "Oi Ferris, o GPUI está muito rápido na renderização do chat!",
        )
        .with_timestamp(now - Duration::minutes(50));
        let msg_df2 = Message::new(
            "msg-df-2",
            "chan-dm-ferris",
            Author::new("user-ferris", "ferris"),
            "Com certeza! Zero flickering e latência sub-milissegundo com aceleração por GPU. 🦀",
        )
        .with_timestamp(now - Duration::minutes(40));
        messages.insert("chan-dm-ferris".to_string(), vec![msg_df1.clone(), msg_df2.clone()]);
        messages.insert("dm-ferris".to_string(), vec![msg_df1, msg_df2]);

        let state = MockState {
            current_user,
            guilds: vec![rust_brasil_guild, gpui_hub_guild],
            messages,
            connected: false,
            next_message_id: 1000,
        };

        Self {
            state: Arc::new(RwLock::new(state)),
            event_sender,
        }
    }

    /// Simulates receiving an external message from another user in real time.
    pub async fn simulate_incoming_message(
        &self,
        channel_id: &str,
        author: Author,
        content: &str,
    ) -> anyhow::Result<Message> {
        let mut state = self.state.write().await;
        let id = format!("msg-sim-{}", state.next_message_id);
        state.next_message_id += 1;

        let mut msg = Message::new(id, channel_id, author, content);
        for guild in &state.guilds {
            if guild.channels.iter().any(|c| c.id == channel_id) {
                msg.guild_id = Some(guild.id.clone());
                break;
            }
        }
        state
            .messages
            .entry(channel_id.to_string())
            .or_default()
            .push(msg.clone());

        let _ = self.event_sender.send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }
}

impl Default for MockDiscordProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DiscordTransport for MockDiscordProvider {
    async fn connect(&self) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.connected = true;

        let _ = self.event_sender.send(TransportEvent::Connected);
        let _ = self.event_sender.send(TransportEvent::Ready {
            user: state.current_user.clone(),
            guilds: state.guilds.clone(),
            guild_folders: Vec::new(),
        });
        Ok(())
    }

    async fn disconnect(&self) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.connected = false;
        let _ = self
            .event_sender
            .send(TransportEvent::Disconnected("Mock provider disconnected".to_string()));
        Ok(())
    }

    async fn get_current_user(&self) -> anyhow::Result<User> {
        let state = self.state.read().await;
        Ok(state.current_user.clone())
    }

    async fn list_guilds(&self) -> anyhow::Result<Vec<Guild>> {
        let state = self.state.read().await;
        Ok(state.guilds.clone())
    }

    async fn get_guild(&self, guild_id: &str) -> anyhow::Result<Option<Guild>> {
        let state = self.state.read().await;
        Ok(state.guilds.iter().find(|g| g.id == guild_id).cloned())
    }

    async fn get_guild_emojis(&self, guild_id: &str) -> anyhow::Result<Vec<DiscordEmoji>> {
        let state = self.state.read().await;
        let guild = state.guilds.iter().find(|g| g.id == guild_id);
        Ok(guild.map(|g| g.emojis.clone()).unwrap_or_default())
    }

    async fn get_guild_stickers(&self, guild_id: &str) -> anyhow::Result<Vec<DiscordSticker>> {
        let state = self.state.read().await;
        let guild = state.guilds.iter().find(|g| g.id == guild_id);
        Ok(guild.map(|g| g.stickers.clone()).unwrap_or_default())
    }

    async fn list_channels(&self, guild_id: &str) -> anyhow::Result<Vec<Channel>> {
        let state = self.state.read().await;
        let guild = state.guilds.iter().find(|g| g.id == guild_id);
        Ok(guild.map(|g| g.channels.clone()).unwrap_or_default())
    }

    async fn list_dm_channels(&self) -> anyhow::Result<Vec<Channel>> {
        let mut ch_ferris = Channel::new(
            "chan-dm-ferris",
            "Ferris",
            crate::domain::ChannelType::Dm,
        );
        ch_ferris.recipient_ids = vec!["user-ferris".to_string()];
        ch_ferris.recipient_names = vec!["Ferris".to_string()];
        ch_ferris.topic = Some("Mensagens Diretas com Ferris".into());
        ch_ferris.last_message_id = Some("msg-df-2".to_string());

        let mut ch_ana = Channel::new(
            "chan-dm-ana",
            "Ana Dev",
            crate::domain::ChannelType::Dm,
        );
        ch_ana.recipient_ids = vec!["user-ana".to_string()];
        ch_ana.recipient_names = vec!["Ana Dev".to_string()];
        ch_ana.topic = Some("Mensagens Diretas com Ana".into());

        Ok(vec![ch_ferris, ch_ana])
    }

    async fn create_or_get_dm_channel(&self, recipient_id: &str) -> anyhow::Result<Channel> {
        let (real_id, name, uid) = match recipient_id {
            "user-ferris" | "ferris" => ("chan-dm-ferris".to_string(), "Ferris", "user-ferris"),
            "user-ana" | "ana" => ("chan-dm-ana".to_string(), "Ana Dev", "user-ana"),
            "user-carlos" | "carlos" => ("chan-dm-carlos".to_string(), "Carlos", "user-carlos"),
            other => {
                let clean = other.trim_start_matches("dm-");
                (format!("chan-dm-{}", clean), clean, other)
            }
        };
        let mut ch = Channel::new(
            real_id,
            name,
            crate::domain::ChannelType::Dm,
        );
        ch.recipient_ids = vec![uid.to_string()];
        ch.recipient_names = vec![name.to_string()];
        ch.topic = Some(format!("Mensagens Diretas com {}", name));
        Ok(ch)
    }

    async fn list_relationships(&self) -> anyhow::Result<Vec<Relationship>> {
        Ok(vec![
            Relationship {
                id: "user-ferris".to_string(),
                rel_type: 1,
                username: "ferris_the_crab".to_string(),
                global_name: Some("Ferris".to_string()),
                avatar: None,
            },
            Relationship {
                id: "user-ana".to_string(),
                rel_type: 1,
                username: "ana_rust".to_string(),
                global_name: Some("Ana Dev".to_string()),
                avatar: None,
            },
            Relationship {
                id: "user-carlos".to_string(),
                rel_type: 1,
                username: "carlos_coder".to_string(),
                global_name: Some("Carlos".to_string()),
                avatar: None,
            },
        ])
    }

    async fn get_messages(&self, channel_id: &str, limit: usize) -> anyhow::Result<Vec<Message>> {
        let state = self.state.read().await;
        let msgs = state.messages.get(channel_id).cloned().or_else(|| {
            if channel_id == "chan-dm-ferris" {
                state.messages.get("dm-ferris").cloned()
            } else if channel_id == "dm-ferris" {
                state.messages.get("chan-dm-ferris").cloned()
            } else if channel_id == "chan-dm-ana" {
                state.messages.get("dm-ana").cloned()
            } else if channel_id == "dm-ana" {
                state.messages.get("chan-dm-ana").cloned()
            } else {
                None
            }
        });
        if let Some(list) = msgs {
            let start = if list.len() > limit {
                list.len() - limit
            } else {
                0
            };
            Ok(list[start..].to_vec())
        } else {
            Ok(Vec::new())
        }
    }

    async fn send_message(&self, channel_id: &str, content: &str) -> anyhow::Result<Message> {
        let mut state = self.state.write().await;
        let id = format!("msg-{}", state.next_message_id);
        state.next_message_id += 1;

        let author = Author {
            id: state.current_user.id.clone(),
            username: state.current_user.username.clone(),
            global_name: state.current_user.global_name.clone(),
            avatar: state.current_user.avatar.clone(),
            bot: state.current_user.bot,
        };

        let mut msg = Message::new(id, channel_id, author, content);
        for guild in &state.guilds {
            if guild.channels.iter().any(|c| c.id == channel_id) {
                msg.guild_id = Some(guild.id.clone());
                break;
            }
        }

        state
            .messages
            .entry(channel_id.to_string())
            .or_default()
            .push(msg.clone());

        if channel_id == "chan-dm-ferris" {
            state.messages.entry("dm-ferris".to_string()).or_default().push(msg.clone());
        } else if channel_id == "dm-ferris" {
            state.messages.entry("chan-dm-ferris".to_string()).or_default().push(msg.clone());
        }

        // Update channel last_message_id in all guilds
        for guild in &mut state.guilds {
            for chan in &mut guild.channels {
                if chan.id == channel_id {
                    chan.last_message_id = Some(msg.id.clone());
                }
            }
        }

        let _ = self.event_sender.send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }

    async fn send_sticker(&self, channel_id: &str, sticker_id: &str) -> anyhow::Result<Message> {
        let mut state = self.state.write().await;
        let id = format!("msg-{}", state.next_message_id);
        state.next_message_id += 1;

        let author = Author {
            id: state.current_user.id.clone(),
            username: state.current_user.username.clone(),
            global_name: state.current_user.global_name.clone(),
            avatar: state.current_user.avatar.clone(),
            bot: state.current_user.bot,
        };

        let mut msg = Message::new(id, channel_id, author, "");
        for guild in &state.guilds {
            if guild.channels.iter().any(|c| c.id == channel_id) {
                msg.guild_id = Some(guild.id.clone());
                break;
            }
        }
        msg.sticker_items.push(MessageStickerItem::new(
            sticker_id,
            "mock-sticker",
            1,
        ));

        state
            .messages
            .entry(channel_id.to_string())
            .or_default()
            .push(msg.clone());

        if channel_id == "chan-dm-ferris" {
            state.messages.entry("dm-ferris".to_string()).or_default().push(msg.clone());
        } else if channel_id == "dm-ferris" {
            state.messages.entry("chan-dm-ferris".to_string()).or_default().push(msg.clone());
        }

        for guild in &mut state.guilds {
            for chan in &mut guild.channels {
                if chan.id == channel_id {
                    chan.last_message_id = Some(msg.id.clone());
                }
            }
        }

        let _ = self.event_sender.send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }

    async fn add_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        let current_user_id = state.current_user.id.clone();

        if let Some(list) = state.messages.get_mut(channel_id) {
            if let Some(msg) = list.iter_mut().find(|m| m.id == message_id) {
                msg.add_reaction(emoji, true);

                let _ = self.event_sender.send(TransportEvent::ReactionAdd {
                    channel_id: channel_id.to_string(),
                    message_id: message_id.to_string(),
                    user_id: current_user_id,
                    emoji: emoji.to_string(),
                });
                return Ok(());
            }
        }
        anyhow::bail!("Message {} not found in channel {}", message_id, channel_id);
    }

    async fn remove_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        let current_user_id = state.current_user.id.clone();

        if let Some(list) = state.messages.get_mut(channel_id) {
            if let Some(msg) = list.iter_mut().find(|m| m.id == message_id) {
                msg.remove_reaction(emoji, true);

                let _ = self.event_sender.send(TransportEvent::ReactionRemove {
                    channel_id: channel_id.to_string(),
                    message_id: message_id.to_string(),
                    user_id: current_user_id,
                    emoji: emoji.to_string(),
                });
                return Ok(());
            }
        }
        anyhow::bail!("Message {} not found in channel {}", message_id, channel_id);
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_sender.subscribe()
    }
}
