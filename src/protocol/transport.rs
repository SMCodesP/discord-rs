use crate::domain::{
    Activity, Channel, CustomStatus, Guild, Message, PresenceStatus, User, VoiceParticipant, VoiceState,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Representation of user guild folders from Discord Gateway user settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GuildFolder {
    pub id: Option<String>,
    pub name: Option<String>,
    pub color: Option<u32>,
    pub guild_ids: Vec<String>,
}

/// Discord user relationship (Friend, Blocked, Incoming/Outgoing Friend Request)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Relationship {
    pub id: String,
    pub rel_type: u32,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

impl Relationship {
    pub const TYPE_FRIEND: u32 = 1;
    pub const TYPE_BLOCKED: u32 = 2;
    pub const TYPE_INCOMING_REQUEST: u32 = 3;
    pub const TYPE_OUTGOING_REQUEST: u32 = 4;

    pub fn is_friend(&self) -> bool {
        self.rel_type == Self::TYPE_FRIEND
    }

    pub fn is_blocked(&self) -> bool {
        self.rel_type == Self::TYPE_BLOCKED
    }

    pub fn is_incoming_request(&self) -> bool {
        self.rel_type == Self::TYPE_INCOMING_REQUEST
    }

    pub fn is_outgoing_request(&self) -> bool {
        self.rel_type == Self::TYPE_OUTGOING_REQUEST
    }
}

/// High-level real-time transport events emitted by the Discord Gateway or Mock provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportEvent {
    /// Dispatched after successful Gateway handshake and READY payload parsing.
    Ready {
        user: User,
        guilds: Vec<Guild>,
        guild_folders: Vec<GuildFolder>,
    },
    /// A new chat message was posted.
    MessageCreate(Message),
    /// A chat message was edited.
    MessageUpdate(Message),
    /// A chat message was deleted.
    MessageDelete {
        channel_id: String,
        message_id: String,
    },
    /// A reaction emoji was added to a message.
    ReactionAdd {
        channel_id: String,
        message_id: String,
        user_id: String,
        emoji: String,
    },
    /// A reaction emoji was removed from a message.
    ReactionRemove {
        channel_id: String,
        message_id: String,
        user_id: String,
        emoji: String,
    },
    /// Presence status or activity for a user changed.
    PresenceUpdate {
        user_id: String,
        status: PresenceStatus,
        activity: Option<Activity>,
        #[serde(default)]
        custom_status: Option<CustomStatus>,
    },
    /// Voice state for a user changed (e.g. joined/left voice channel, mute, deaf).
    VoiceStateUpdate(VoiceState),
    /// Participant status updated in an active voice room.
    VoiceParticipantUpdate(VoiceParticipant),
    /// Voice server update dispatched by main Discord Gateway for establishing voice connection
    VoiceServerUpdate {
        token: String,
        guild_id: Option<String>,
        channel_id: Option<String>,
        endpoint: Option<String>,
    },
    /// A user started typing in a channel.
    TypingStart {
        channel_id: String,
        user_id: String,
        timestamp: u64,
    },
    /// A new channel was created.
    ChannelCreate(Channel),
    /// A new guild was joined or became available.
    GuildCreate(Guild),
    /// Transport successfully connected.
    Connected,
    /// Transport disconnected or lost connection.
    Disconnected(String),
    /// Channels updated for a specific guild (lazy loading / on-demand fetch)
    GuildChannelsUpdated {
        guild_id: String,
        channels: Vec<Channel>,
    },
    /// Direct Message (DM) channels updated for current user
    DmChannelsUpdated(Vec<Channel>),
    /// Direct Message (DM) channel created or opened in real-time
    DmChannelCreated(Channel),
    /// Direct Message (DM) channel successfully resolved from temporary ID to live Discord channel
    DmChannelResolved {
        temp_id: String,
        channel: Channel,
    },
    /// User relationships / friends list updated
    RelationshipsUpdated(Vec<Relationship>),
    /// User relationship added or updated in real-time
    RelationshipAdd(Relationship),
    /// User relationship removed in real-time
    RelationshipRemove {
        id: String,
    },
    /// Friend request outcome notification (success/error message)
    FriendRequestResult {
        username: String,
        success: bool,
        message: String,
    },
    /// Relationship action (accept, decline, remove) failed
    RelationshipActionFailed {
        id: String,
        error: String,
    },
    /// Batch message history loaded for a channel (avoids flooding individual MessageCreate events)
    ChannelMessagesLoaded {
        channel_id: String,
        messages: Vec<Message>,
    },
    /// Custom emojis updated for a guild
    GuildEmojisUpdated {
        guild_id: String,
        emojis: Vec<crate::protocol::media_cache::DiscordEmoji>,
    },
    /// Custom stickers updated for a guild
    GuildStickersUpdated {
        guild_id: String,
        stickers: Vec<crate::protocol::media_cache::DiscordSticker>,
    },
    /// Pinned direct message (DM) channel IDs updated from user settings or cache
    PinnedDmsUpdated(Vec<String>),
}

/// Asynchronous transport abstraction for Discord communication.
///
/// Enables seamless switching between deterministic mock providers for testing/UI prototyping
/// and live Discord API (REST v10 + Gateway v10 WebSocket) clients.
#[async_trait]
pub trait DiscordTransport: Send + Sync {
    /// Establish gateway connection and start background event loops.
    async fn connect(&self) -> anyhow::Result<()>;

    /// Gracefully disconnect and cancel background tasks.
    async fn disconnect(&self) -> anyhow::Result<()>;

    /// Retrieve the current authenticated user identity (@me).
    async fn get_current_user(&self) -> anyhow::Result<User>;

    /// Fetch all available guilds / servers for the authenticated identity.
    async fn list_guilds(&self) -> anyhow::Result<Vec<Guild>>;

    /// Retrieve detailed guild information by guild ID.
    async fn get_guild(&self, guild_id: &str) -> anyhow::Result<Option<Guild>>;

    /// Fetch all custom emojis for a guild (/guilds/{guild_id}/emojis).
    async fn get_guild_emojis(&self, guild_id: &str) -> anyhow::Result<Vec<crate::protocol::media_cache::DiscordEmoji>> {
        let _ = guild_id;
        Ok(Vec::new())
    }

    /// Fetch all custom stickers for a guild (/guilds/{guild_id}/stickers).
    async fn get_guild_stickers(&self, guild_id: &str) -> anyhow::Result<Vec<crate::protocol::media_cache::DiscordSticker>> {
        let _ = guild_id;
        Ok(Vec::new())
    }

    /// List all channels within a guild.
    async fn list_channels(&self, guild_id: &str) -> anyhow::Result<Vec<Channel>>;

    /// List all direct message (DM) channels for the authenticated user (/users/@me/channels).
    async fn list_dm_channels(&self) -> anyhow::Result<Vec<Channel>>;

    /// Create or retrieve an existing direct message channel with a user (/users/@me/channels).
    async fn create_or_get_dm_channel(&self, recipient_id: &str) -> anyhow::Result<Channel> {
        let _ = recipient_id;
        anyhow::bail!("create_or_get_dm_channel not supported on this transport");
    }

    /// List all user relationships (friends, incoming/outgoing requests, blocked) (/users/@me/relationships).
    async fn list_relationships(&self) -> anyhow::Result<Vec<Relationship>> {
        Ok(Vec::new())
    }

    /// Send a friend request to a user (/users/@me/relationships)
    async fn send_friend_request(&self, username: &str, discriminator: Option<&str>) -> anyhow::Result<()> {
        let _ = (username, discriminator);
        anyhow::bail!("send_friend_request not supported on this transport");
    }

    /// Accept a pending friend request (/users/@me/relationships/{user_id})
    async fn accept_friend_request(&self, user_id: &str) -> anyhow::Result<()> {
        let _ = user_id;
        anyhow::bail!("accept_friend_request not supported on this transport");
    }

    /// Remove, decline, unblock or cancel a relationship (/users/@me/relationships/{user_id})
    async fn remove_relationship(&self, user_id: &str) -> anyhow::Result<()> {
        let _ = user_id;
        anyhow::bail!("remove_relationship not supported on this transport");
    }

    /// Retrieve the most recent messages for a channel.
    async fn get_messages(&self, channel_id: &str, limit: usize) -> anyhow::Result<Vec<Message>>;

    /// Post a new chat message to a text channel.
    async fn send_message(&self, channel_id: &str, content: &str) -> anyhow::Result<Message>;

    /// Post a sticker message to a text channel.
    async fn send_sticker(&self, channel_id: &str, sticker_id: &str) -> anyhow::Result<Message>;

    /// Add an emoji reaction to a message.
    async fn add_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> anyhow::Result<()>;

    /// Remove an emoji reaction from a message.
    async fn remove_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> anyhow::Result<()>;

    /// Send Opcode 4 Voice State Update to Discord Gateway.
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> anyhow::Result<()> {
        let _ = (guild_id, channel_id, self_mute, self_deaf);
        Ok(())
    }

    /// Ring participants in a DM / private channel via POST /channels/{id}/call/ring.
    async fn ring_call(
        &self,
        channel_id: &str,
        recipients: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let _ = (channel_id, recipients);
        Ok(())
    }

    /// Subscribe to real-time events emitted by the transport.
    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<TransportEvent>;
}
