//! Lazy Guilds & Opcode 14 (Guild Subscriptions)
//!
//! Garante que mensagens e membros nunca sejam puxados de todos os servidores simultaneamente.
//! O cliente assina sob demanda exclusivamente o servidor e canal em foco no viewport.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Faixa de índices de membros visíveis na sidebar de membros: `[start_index, end_index]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRange(pub usize, pub usize);

/// Payload do Opcode 14 (Guild Subscriptions) do Gateway Discord
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildSubscription {
    pub guild_id: String,
    pub typing: bool,
    pub threads: bool,
    pub activities: bool,
    pub members: Vec<String>,
    pub channels: HashMap<String, Vec<[usize; 2]>>,
}

/// Coordenador de Lazy Guilds gerenciando o estado de subscrições ativas
#[derive(Debug, Clone)]
pub struct LazyGuildCoordinator {
    active_guild_id: Arc<RwLock<Option<String>>>,
    active_channel_id: Arc<RwLock<Option<String>>>,
    channel_ranges: Arc<RwLock<HashMap<String, Vec<[usize; 2]>>>>,
}

impl Default for LazyGuildCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

static SHARED_COORDINATOR: std::sync::OnceLock<LazyGuildCoordinator> = std::sync::OnceLock::new();

impl LazyGuildCoordinator {
    /// Returns the global shared `LazyGuildCoordinator` instance alive per session.
    pub fn shared() -> &'static LazyGuildCoordinator {
        SHARED_COORDINATOR.get_or_init(LazyGuildCoordinator::new)
    }

    pub fn new() -> Self {
        Self {
            active_guild_id: Arc::new(RwLock::new(None)),
            active_channel_id: Arc::new(RwLock::new(None)),
            channel_ranges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Retorna as faixas ativas para um canal específico
    pub async fn active_ranges_for_channel(&self, channel_id: &str) -> Vec<[usize; 2]> {
        let map = self.channel_ranges.read().await;
        map.get(channel_id).cloned().unwrap_or_else(|| vec![[0, 99]])
    }

    /// Reinicia as faixas para um canal (ex.: troca de canal)
    pub async fn reset_channel_ranges(&self, channel_id: &str) {
        let mut map = self.channel_ranges.write().await;
        map.insert(channel_id.to_string(), vec![[0, 99]]);
    }

    /// Limpa o estado do coordenador (ex.: desconexão ou logout)
    pub async fn clear(&self) {
        *self.active_guild_id.write().await = None;
        *self.active_channel_id.write().await = None;
        self.channel_ranges.write().await.clear();
    }

    /// Retorna o ID da guilda atualmente ativa
    pub async fn active_guild_id(&self) -> Option<String> {
        self.active_guild_id.read().await.clone()
    }

    /// Retorna o ID do canal atualmente ativo
    pub async fn active_channel_id(&self) -> Option<String> {
        self.active_channel_id.read().await.clone()
    }

    /// Alterna o foco para um novo servidor e canal inicial, gerando o payload Op 14
    pub async fn focus_guild_and_channel(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> serde_json::Value {
        {
            let mut g = self.active_guild_id.write().await;
            *g = Some(guild_id.to_string());
        }
        {
            let mut c = self.active_channel_id.write().await;
            *c = Some(channel_id.to_string());
        }

        // Padrão oficial: primeiros 100 membros no topo do viewport `[[0, 99]]`
        let default_range = vec![[0, 99]];
        {
            let mut ranges = self.channel_ranges.write().await;
            ranges.clear();
            ranges.insert(channel_id.to_string(), default_range.clone());
        }

        self.generate_opcode_14_payload(guild_id, channel_id, default_range)
    }

    /// Atualiza as faixas de membros visíveis (por exemplo durante scroll na sidebar)
    pub async fn update_viewport_ranges(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> serde_json::Value {
        {
            let mut map = self.channel_ranges.write().await;
            map.insert(channel_id.to_string(), ranges.clone());
        }

        self.generate_opcode_14_payload(guild_id, channel_id, ranges)
    }

    /// Constrói a mensagem WebSocket JSON Op 14 formatada exatamente conforme o protocolo Discord
    fn generate_opcode_14_payload(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> serde_json::Value {
        let mut channels_map = HashMap::new();
        channels_map.insert(channel_id.to_string(), ranges);

        json!({
            "op": 14,
            "d": {
                "guild_id": guild_id,
                "typing": true,
                "threads": true,
                "activities": true,
                "members": [],
                "channels": channels_map
            }
        })
    }
}

// ============================================================================
// GUILD_MEMBER_LIST_UPDATE Protocol Data Structures & Parser
// ============================================================================

use crate::app::state::{MemberItemState, PresenceStatus};
use crate::domain::guild::Role;
use crate::domain::user::AvatarDecorationData;

/// Represents a parsed group header inside a GUILD_MEMBER_LIST_UPDATE event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberListGroup {
    pub id: String,
    pub count: usize,
}

/// Represents an individual member received in a GUILD_MEMBER_LIST_UPDATE event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberListMember {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    pub roles: Vec<String>,
    pub presence: PresenceStatus,
    pub activity: Option<String>,
    pub is_bot: bool,
}

/// An item inside an operation: either a role group header or a member
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberListItem {
    Group(MemberListGroup),
    Member(Box<MemberListMember>),
}

/// An operation inside GUILD_MEMBER_LIST_UPDATE
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberListOp {
    Sync {
        range: [usize; 2],
        items: Vec<MemberListItem>,
    },
    Insert {
        index: usize,
        item: MemberListItem,
    },
    Update {
        index: usize,
        item: MemberListItem,
    },
    Delete {
        index: usize,
    },
    Invalidate {
        range: [usize; 2],
    },
}

/// Parsed `GUILD_MEMBER_LIST_UPDATE` Discord Gateway event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildMemberListUpdate {
    pub guild_id: String,
    pub id: String,
    pub member_count: Option<usize>,
    pub online_count: Option<usize>,
    pub groups: Vec<MemberListGroup>,
    pub ops: Vec<MemberListOp>,
}

/// Resolves the canonical role color for a member given their assigned role IDs and the guild's roles.
/// Sorts matched roles by `position` descending and returns the color of the highest role with non-zero color.
pub fn resolve_member_role_color(member_roles: &[String], guild_roles: &[Role]) -> u32 {
    let mut matched: Vec<&Role> = guild_roles
        .iter()
        .filter(|r| member_roles.iter().any(|rid| rid == &r.id))
        .collect();

    // Sort by role hierarchy position descending (highest role wins)
    matched.sort_by(|a, b| b.position.cmp(&a.position));

    for role in matched {
        if role.color != 0 {
            return role.color;
        }
    }
    0
}

/// Resolves the hoisted group name for a member given their assigned role IDs and the guild's roles.
/// Sorts hoisted roles by `position` descending and returns the highest role's name.
pub fn resolve_member_role_group(member_roles: &[String], guild_roles: &[Role]) -> Option<String> {
    let mut matched: Vec<&Role> = guild_roles
        .iter()
        .filter(|r| r.hoist && member_roles.iter().any(|rid| rid == &r.id))
        .collect();

    matched.sort_by(|a, b| b.position.cmp(&a.position));

    matched.first().map(|r| r.name.clone())
}

/// Parses a `GUILD_MEMBER_LIST_UPDATE` Gateway event from raw JSON
pub fn parse_guild_member_list_update(val: &serde_json::Value) -> Option<GuildMemberListUpdate> {
    let d = val.get("d").unwrap_or(val);
    let guild_id = d.get("guild_id")?.as_str()?.to_string();
    let id = d.get("id").and_then(|v| v.as_str()).unwrap_or("everyone").to_string();
    let member_count = d.get("member_count").and_then(|v| v.as_u64()).map(|v| v as usize);
    let online_count = d.get("online_count").and_then(|v| v.as_u64()).map(|v| v as usize);

    let mut groups = Vec::new();
    if let Some(groups_arr) = d.get("groups").and_then(|v| v.as_array()) {
        for g in groups_arr {
            if let (Some(gid), Some(cnt)) = (
                g.get("id").and_then(|v| v.as_str()),
                g.get("count").and_then(|v| v.as_u64()),
            ) {
                groups.push(MemberListGroup {
                    id: gid.to_string(),
                    count: cnt as usize,
                });
            }
        }
    }

    let mut ops = Vec::new();
    if let Some(ops_arr) = d.get("ops").and_then(|v| v.as_array()) {
        for op_val in ops_arr {
            let op_type = op_val.get("op").and_then(|v| v.as_str()).unwrap_or("");
            match op_type {
                "SYNC" => {
                    let range_arr = op_val.get("range").and_then(|v| v.as_array());
                    let range = if let Some(r) = range_arr {
                        if r.len() >= 2 {
                            [
                                r[0].as_u64().unwrap_or(0) as usize,
                                r[1].as_u64().unwrap_or(99) as usize,
                            ]
                        } else {
                            [0, 99]
                        }
                    } else {
                        [0, 99]
                    };

                    let mut items = Vec::new();
                    if let Some(items_arr) = op_val.get("items").and_then(|v| v.as_array()) {
                        for it in items_arr {
                            if let Some(group_obj) = it.get("group") {
                                if let (Some(gid), Some(cnt)) = (
                                    group_obj.get("id").and_then(|v| v.as_str()),
                                    group_obj.get("count").and_then(|v| v.as_u64()),
                                ) {
                                    items.push(MemberListItem::Group(MemberListGroup {
                                        id: gid.to_string(),
                                        count: cnt as usize,
                                    }));
                                }
                            } else if let Some(member_obj) = it.get("member") {
                                if let Some(parsed_m) = parse_member_obj(member_obj) {
                                    items.push(MemberListItem::Member(Box::new(parsed_m)));
                                }
                            }
                        }
                    }
                    ops.push(MemberListOp::Sync { range, items });
                }
                "INSERT" => {
                    let index = op_val.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if let Some(item_val) = op_val.get("item") {
                        if let Some(group_obj) = item_val.get("group") {
                            if let (Some(gid), Some(cnt)) = (
                                group_obj.get("id").and_then(|v| v.as_str()),
                                group_obj.get("count").and_then(|v| v.as_u64()),
                            ) {
                                ops.push(MemberListOp::Insert {
                                    index,
                                    item: MemberListItem::Group(MemberListGroup {
                                        id: gid.to_string(),
                                        count: cnt as usize,
                                    }),
                                });
                            }
                        } else if let Some(member_obj) = item_val.get("member") {
                            if let Some(parsed_m) = parse_member_obj(member_obj) {
                                ops.push(MemberListOp::Insert {
                                    index,
                                    item: MemberListItem::Member(Box::new(parsed_m)),
                                });
                            }
                        }
                    }
                }
                "UPDATE" => {
                    let index = op_val.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if let Some(item_val) = op_val.get("item") {
                        if let Some(member_obj) = item_val.get("member") {
                            if let Some(parsed_m) = parse_member_obj(member_obj) {
                                ops.push(MemberListOp::Update {
                                    index,
                                    item: MemberListItem::Member(Box::new(parsed_m)),
                                });
                            }
                        }
                    }
                }
                "DELETE" => {
                    let index = op_val.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    ops.push(MemberListOp::Delete { index });
                }
                "INVALIDATE" => {
                    let range_arr = op_val.get("range").and_then(|v| v.as_array());
                    let range = if let Some(r) = range_arr {
                        if r.len() >= 2 {
                            [
                                r[0].as_u64().unwrap_or(0) as usize,
                                r[1].as_u64().unwrap_or(99) as usize,
                            ]
                        } else {
                            [0, 99]
                        }
                    } else {
                        [0, 99]
                    };
                    ops.push(MemberListOp::Invalidate { range });
                }
                _ => {}
            }
        }
    }

    Some(GuildMemberListUpdate {
        guild_id,
        id,
        member_count,
        online_count,
        groups,
        ops,
    })
}

fn parse_member_obj(val: &serde_json::Value) -> Option<MemberListMember> {
    let user = val.get("user")?;
    let id = user.get("id")?.as_str()?.to_string();
    let username = user.get("username")?.as_str()?.to_string();
    let global_name = user.get("global_name").and_then(|v| v.as_str()).map(ToString::to_string);
    let avatar = user.get("avatar").and_then(|v| v.as_str()).map(ToString::to_string);
    let is_bot = user.get("bot").and_then(|v| v.as_bool()).unwrap_or(false);

    let avatar_decoration_data = user.get("avatar_decoration_data").and_then(|v| {
        serde_json::from_value(v.clone()).ok()
    });

    let roles: Vec<String> = val
        .get("roles")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    let presence = if let Some(p) = val.get("presence") {
        match p.get("status").and_then(|v| v.as_str()).unwrap_or("offline") {
            "online" => PresenceStatus::Online,
            "idle" => PresenceStatus::Idle,
            "dnd" => PresenceStatus::DoNotDisturb,
            _ => PresenceStatus::Offline,
        }
    } else {
        PresenceStatus::Offline
    };

    let activity = val.get("presence").and_then(|p| {
        p.get("activities")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|act| act.get("name").and_then(|v| v.as_str()))
            .map(ToString::to_string)
    });

    Some(MemberListMember {
        id,
        username,
        global_name,
        avatar,
        avatar_decoration_data,
        roles,
        presence,
        activity,
        is_bot,
    })
}

/// Converts a `MemberListMember` into a `MemberItemState` resolving role color and group
pub fn convert_member_to_item_state(
    member: &MemberListMember,
    guild_roles: &[Role],
) -> MemberItemState {
    let display = member.global_name.as_deref().unwrap_or(&member.username);
    let avatar_text = display.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string());
    let role_color = resolve_member_role_color(&member.roles, guild_roles);
    let role_group = resolve_member_role_group(&member.roles, guild_roles)
        .unwrap_or_else(|| {
            if member.presence == PresenceStatus::Offline {
                "OFFLINE".to_string()
            } else if member.is_bot {
                "BOTS".to_string()
            } else {
                "DISPONÍVEL".to_string()
            }
        });

    MemberItemState {
        id: member.id.clone(),
        name: display.to_string(),
        avatar_text,
        presence: member.presence,
        role_group,
        role_color,
        is_bot: member.is_bot,
        activity: member.activity.clone(),
        avatar_decoration_data: member.avatar_decoration_data.clone(),
    }
}

use std::sync::OnceLock;
use std::sync::RwLock as StdRwLock;

static AUTHOR_ROLES_CACHE: OnceLock<StdRwLock<HashMap<(String, String), Vec<String>>>> = OnceLock::new();
static AUTHOR_ROLE_COLOR_CACHE: OnceLock<StdRwLock<HashMap<(String, String), u32>>> = OnceLock::new();
static GUILD_ROLES_CACHE: OnceLock<StdRwLock<HashMap<String, Vec<Role>>>> = OnceLock::new();

fn roles_cache() -> &'static StdRwLock<HashMap<(String, String), Vec<String>>> {
    AUTHOR_ROLES_CACHE.get_or_init(|| StdRwLock::new(HashMap::new()))
}

fn role_color_cache() -> &'static StdRwLock<HashMap<(String, String), u32>> {
    AUTHOR_ROLE_COLOR_CACHE.get_or_init(|| StdRwLock::new(HashMap::new()))
}

fn guild_roles_cache() -> &'static StdRwLock<HashMap<String, Vec<Role>>> {
    GUILD_ROLES_CACHE.get_or_init(|| StdRwLock::new(HashMap::new()))
}

pub fn cache_guild_roles(guild_id: &str, roles: Vec<Role>) {
    if let Ok(mut lock) = guild_roles_cache().write() {
        lock.insert(guild_id.to_string(), roles);
    }
}

pub fn get_cached_guild_roles(guild_id: &str) -> Vec<Role> {
    let lock = guild_roles_cache().read().ok();
    lock.and_then(|l| l.get(guild_id).cloned()).unwrap_or_default()
}

pub fn cache_author_roles(guild_id: &str, user_id: &str, roles: Vec<String>) {
    if let Ok(mut lock) = roles_cache().write() {
        lock.insert((guild_id.to_string(), user_id.to_string()), roles);
    }
}

pub fn get_cached_author_roles(guild_id: &str, user_id: &str) -> Option<Vec<String>> {
    let lock = roles_cache().read().ok()?;
    lock.get(&(guild_id.to_string(), user_id.to_string())).cloned()
}

pub fn cache_author_role_color(guild_id: &str, user_id: &str, color: u32) {
    if let Ok(mut lock) = role_color_cache().write() {
        lock.insert((guild_id.to_string(), user_id.to_string()), color);
    }
}

pub fn get_cached_author_role_color(guild_id: &str, user_id: &str) -> Option<u32> {
    let lock = role_color_cache().read().ok()?;
    lock.get(&(guild_id.to_string(), user_id.to_string())).copied()
}

/// Converts a `MemberListItem` (Group or Member) into a `MemberItemState`
pub fn convert_list_item_to_state(
    item: &MemberListItem,
    guild_id: &str,
    guild_roles: &[Role],
) -> MemberItemState {
    match item {
        MemberListItem::Group(g) => {
            let (title, role_color) = if g.id == "online" {
                ("ONLINE".to_string(), 0)
            } else if g.id == "offline" {
                ("OFFLINE".to_string(), 0)
            } else if let Some(role) = guild_roles.iter().find(|r| r.id == g.id) {
                (role.name.clone(), role.color)
            } else {
                (g.id.clone(), 0)
            };

            MemberItemState {
                id: format!("group:{}", g.id),
                name: title,
                avatar_text: String::new(),
                presence: if g.id == "offline" {
                    PresenceStatus::Offline
                } else {
                    PresenceStatus::Online
                },
                role_group: g.id.clone(),
                role_color,
                is_bot: false,
                activity: Some(g.count.to_string()),
                avatar_decoration_data: None,
            }
        }
        MemberListItem::Member(m) => {
            let state = convert_member_to_item_state(m, guild_roles);
            if state.role_color != 0 {
                cache_author_role_color(guild_id, &m.id, state.role_color);
            }
            cache_author_roles(guild_id, &m.id, m.roles.clone());
            state
        }
    }
}

/// Calculates the member list ranges for Opcode 14 based on the visible viewport indices.
/// Discord always includes the first chunk `[0, 99]` and the chunks covering the visible window.
pub fn calculate_ranges_for_viewport(visible_start: usize, visible_end: usize) -> Vec<[usize; 2]> {
    let mut ranges = Vec::new();
    ranges.push([0, 99]);

    let chunk_size = 100;
    let min_chunk = visible_start / chunk_size;
    let max_chunk = visible_end / chunk_size;

    for chunk in min_chunk..=max_chunk {
        let r = [chunk * chunk_size, (chunk + 1) * chunk_size - 1];
        if !ranges.contains(&r) {
            ranges.push(r);
        }
    }

    ranges.sort_by_key(|r| r[0]);
    ranges
}

/// Applies parsed `GuildMemberListUpdate` operations into the active member list
pub fn apply_member_list_update(
    existing_members: &mut Vec<MemberItemState>,
    update: &GuildMemberListUpdate,
    guild_roles: &[Role],
) {
    if update.ops.is_empty() {
        existing_members.clear();
        return;
    }

    for op in &update.ops {
        match op {
            MemberListOp::Sync { range, items } => {
                let converted: Vec<MemberItemState> = items
                    .iter()
                    .map(|it| convert_list_item_to_state(it, &update.guild_id, guild_roles))
                    .collect();

                let start = range[0];
                // Pad any gap between current length and start with unloaded slots
                if existing_members.len() < start {
                    for i in existing_members.len()..start {
                        existing_members.push(MemberItemState::unloaded(i));
                    }
                }

                for (offset, item) in converted.into_iter().enumerate() {
                    let idx = start + offset;
                    if idx < existing_members.len() {
                        existing_members[idx] = item;
                    } else {
                        existing_members.push(item);
                    }
                }
            }
            MemberListOp::Insert { index, item } => {
                let state = convert_list_item_to_state(item, &update.guild_id, guild_roles);
                let idx = *index;
                if idx < existing_members.len() {
                    existing_members.insert(idx, state);
                } else if idx == existing_members.len() {
                    existing_members.push(state);
                } else {
                    for i in existing_members.len()..idx {
                        existing_members.push(MemberItemState::unloaded(i));
                    }
                    existing_members.push(state);
                }
            }
            MemberListOp::Update { index, item } => {
                let state = convert_list_item_to_state(item, &update.guild_id, guild_roles);
                let idx = *index;
                if idx < existing_members.len() {
                    existing_members[idx] = state;
                } else {
                    for i in existing_members.len()..idx {
                        existing_members.push(MemberItemState::unloaded(i));
                    }
                    existing_members.push(state);
                }
            }
            MemberListOp::Delete { index } => {
                let idx = *index;
                if idx < existing_members.len() {
                    existing_members.remove(idx);
                }
            }
            MemberListOp::Invalidate { range } => {
                let start = range[0];
                let end = range[1];
                if start == 0 && (end + 1) >= existing_members.len() {
                    existing_members.clear();
                } else if start < existing_members.len() {
                    let end_idx = (end + 1).min(existing_members.len());
                    existing_members.drain(start..end_idx);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lazy_guild_opcode_14_generation() {
        let coordinator = LazyGuildCoordinator::new();

        let payload = coordinator
            .focus_guild_and_channel("guild_rust_brasil", "chan_geral")
            .await;

        assert_eq!(payload.get("op").and_then(|v| v.as_u64()), Some(14));

        let d = payload.get("d").expect("d field must exist");
        assert_eq!(
            d.get("guild_id").and_then(|v| v.as_str()),
            Some("guild_rust_brasil")
        );
        assert_eq!(d.get("typing").and_then(|v| v.as_bool()), Some(true));

        let channels = d.get("channels").expect("channels field must exist");
        let general_ranges = channels.get("chan_geral").expect("chan_geral ranges");
        let first_range = &general_ranges.as_array().unwrap()[0];
        assert_eq!(first_range[0].as_u64(), Some(0));
        assert_eq!(first_range[1].as_u64(), Some(99));
    }

    #[test]
    fn test_resolve_member_role_color_hierarchy() {
        let roles = vec![
            Role::new("r1", "Admin").with_color(0xe91e63).with_position(10),
            Role::new("r2", "Mod").with_color(0x2ecc71).with_position(5),
            Role::new("r3", "Vip").with_color(0x3498db).with_position(3),
            Role::new("r0", "NoColor").with_color(0).with_position(20),
        ];

        // Has NoColor (pos 20) and Mod (pos 5) -> returns Mod's color (0x2ecc71) because NoColor has color 0
        let color = resolve_member_role_color(&["r0".into(), "r2".into()], &roles);
        assert_eq!(color, 0x2ecc71);

        // Has Admin (pos 10) and Mod (pos 5) -> returns Admin's color (0xe91e63) because pos 10 > 5
        let color_admin = resolve_member_role_color(&["r1".into(), "r2".into()], &roles);
        assert_eq!(color_admin, 0xe91e63);

        // Has no roles with color -> 0
        let color_none = resolve_member_role_color(&["r0".into()], &roles);
        assert_eq!(color_none, 0);
    }

    #[test]
    fn test_parse_guild_member_list_update() {
        let raw = json!({
            "op": 0,
            "t": "GUILD_MEMBER_LIST_UPDATE",
            "d": {
                "guild_id": "guild_123",
                "id": "everyone",
                "member_count": 42,
                "online_count": 10,
                "groups": [
                    { "id": "admin_role", "count": 2 },
                    { "id": "online", "count": 8 }
                ],
                "ops": [
                    {
                        "op": "SYNC",
                        "range": [0, 99],
                        "items": [
                            { "group": { "id": "admin_role", "count": 2 } },
                            {
                                "member": {
                                    "user": {
                                        "id": "u1",
                                        "username": "ferris",
                                        "global_name": "Ferris",
                                        "avatar": null,
                                        "bot": false
                                    },
                                    "roles": ["r_admin"],
                                    "presence": { "status": "online" }
                                }
                            }
                        ]
                    }
                ]
            }
        });

        let parsed = parse_guild_member_list_update(&raw).expect("Must parse successfully");
        assert_eq!(parsed.guild_id, "guild_123");
        assert_eq!(parsed.member_count, Some(42));
        assert_eq!(parsed.groups.len(), 2);
        assert_eq!(parsed.ops.len(), 1);

        if let MemberListOp::Sync { range, items } = &parsed.ops[0] {
            assert_eq!(*range, [0, 99]);
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], MemberListItem::Group(_)));
            if let MemberListItem::Member(m) = &items[1] {
                assert_eq!(m.id, "u1");
                assert_eq!(m.username, "ferris");
                assert_eq!(m.presence, PresenceStatus::Online);
            } else {
                panic!("Expected Member item");
            }
        } else {
            panic!("Expected Sync op");
        }
    }
}
