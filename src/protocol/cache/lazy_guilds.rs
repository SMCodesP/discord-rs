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

#[derive(Debug, Default)]
struct CoordinatorState {
    active_guild_id: Option<String>,
    active_channel_id: Option<String>,
    channel_ranges: HashMap<String, Vec<[usize; 2]>>,
    generation: u64,
}

/// Coordenador de Lazy Guilds gerenciando o estado de subscrições ativas.
/// Todas as transições usam um único lock para evitar guilda/canal de gerações
/// diferentes aparecerem combinados.
#[derive(Debug, Clone)]
pub struct LazyGuildCoordinator {
    state: Arc<RwLock<CoordinatorState>>,
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
            state: Arc::new(RwLock::new(CoordinatorState::default())),
        }
    }

    /// Retorna as faixas ativas para um canal específico.
    pub async fn active_ranges_for_channel(&self, channel_id: &str) -> Vec<[usize; 2]> {
        let state = self.state.read().await;
        state
            .channel_ranges
            .get(channel_id)
            .cloned()
            .unwrap_or_else(|| vec![[0, 99]])
    }

    /// Reinicia as faixas para um canal (ex.: troca de canal).
    pub async fn reset_channel_ranges(&self, channel_id: &str) {
        let mut state = self.state.write().await;
        if state.active_channel_id.as_deref() == Some(channel_id)
            || state.channel_ranges.contains_key(channel_id)
        {
            state
                .channel_ranges
                .insert(channel_id.to_string(), vec![[0, 99]]);
            state.generation = state.generation.wrapping_add(1);
        }
    }

    /// Limpa o estado do coordenador (ex.: desconexão ou logout).
    pub async fn clear(&self) {
        let previous_generation = {
            let mut state = self.state.write().await;
            let generation = state.generation.wrapping_add(1);
            *state = CoordinatorState {
                generation,
                ..CoordinatorState::default()
            };
            generation
        };
        // Evita que a geração volte a zero após wrap e confunda resultados antigos.
        if previous_generation == 0 {
            let mut state = self.state.write().await;
            state.generation = 1;
        }
        clear_role_caches();
        clear_op8_requested();
    }

    /// Retorna o ID da guilda atualmente ativa.
    pub async fn active_guild_id(&self) -> Option<String> {
        self.state.read().await.active_guild_id.clone()
    }

    /// Retorna o ID do canal atualmente ativo.
    pub async fn active_channel_id(&self) -> Option<String> {
        self.state.read().await.active_channel_id.clone()
    }

    /// Alterna o foco para um novo servidor e canal inicial, gerando o payload Op 14.
    pub async fn focus_guild_and_channel(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> serde_json::Value {
        let default_range = vec![[0, 99]];
        let mut state = self.state.write().await;
        state.active_guild_id = Some(guild_id.to_string());
        state.active_channel_id = Some(channel_id.to_string());
        state.channel_ranges.clear();
        state
            .channel_ranges
            .insert(channel_id.to_string(), default_range.clone());
        state.generation = state.generation.wrapping_add(1);

        Self::generate_opcode_14_payload(guild_id, channel_id, default_range)
    }

    /// Atualiza as faixas somente se guilda/canal ainda forem os ativos.
    /// `None` significa que o resultado está obsoleto e não deve ser enviado.
    pub async fn try_update_viewport_ranges(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> Option<serde_json::Value> {
        let mut state = self.state.write().await;
        if state.active_guild_id.as_deref() != Some(guild_id)
            || state.active_channel_id.as_deref() != Some(channel_id)
        {
            return None;
        }
        state
            .channel_ranges
            .insert(channel_id.to_string(), ranges.clone());
        state.generation = state.generation.wrapping_add(1);
        Some(Self::generate_opcode_14_payload(
            guild_id, channel_id, ranges,
        ))
    }

    /// Atualiza e envia sob o mesmo lock, garantindo que um foco novo não seja
    /// seguido por um Opcode 14 antigo já agendado.
    pub async fn try_update_viewport_ranges_and_send<F, Fut>(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
        send: F,
    ) -> Option<anyhow::Result<()>>
    where
        F: FnOnce(Arc<serde_json::Value>) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<()>> + 'static,
    {
        let mut state = self.state.write().await;
        if state.active_guild_id.as_deref() != Some(guild_id)
            || state.active_channel_id.as_deref() != Some(channel_id)
        {
            return None;
        }
        state
            .channel_ranges
            .insert(channel_id.to_string(), ranges.clone());
        state.generation = state.generation.wrapping_add(1);
        let payload = Arc::new(Self::generate_opcode_14_payload(
            guild_id, channel_id, ranges,
        ));
        Some(send(payload).await)
    }

    /// Compatibilidade com testes/callers legados. Novos envios devem usar
    /// `try_update_viewport_ranges` para descartar gerações obsoletas.
    pub async fn update_viewport_ranges(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> serde_json::Value {
        self.try_update_viewport_ranges(guild_id, channel_id, ranges)
            .await
            .unwrap_or_else(|| json!({}))
    }

    /// Constrói a mensagem WebSocket JSON Op 14 formatada conforme o protocolo Discord.
    fn generate_opcode_14_payload(
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

/// Represents an item received in a GUILD_MEMBERS_CHUNK event (Opcode 8)
/// It carries ONLY what was returned by the gateway (no fake presence or activity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildMemberChunkItem {
    pub id: String,
    pub roles: Vec<String>,
    pub role_color: u32,
    pub username: Option<String>,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub avatar_decoration_data: Option<AvatarDecorationData>,
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

/// Parses a `GUILD_MEMBER_LIST_UPDATE` Gateway event from raw JSON.
/// Eventos com índice/range inválido são rejeitados como um todo.
pub fn parse_guild_member_list_update(val: &serde_json::Value) -> Option<GuildMemberListUpdate> {
    fn parse_range(value: Option<&serde_json::Value>) -> Option<[usize; 2]> {
        let values = value?.as_array()?;
        if values.len() < 2 {
            return None;
        }
        let start = usize::try_from(values[0].as_u64()?).ok()?;
        let end = usize::try_from(values[1].as_u64()?).ok()?;
        (start <= end).then_some([start, end])
    }

    fn parse_index(value: Option<&serde_json::Value>) -> Option<usize> {
        usize::try_from(value?.as_u64()?).ok()
    }

    fn parse_item(value: &serde_json::Value) -> Option<MemberListItem> {
        if let Some(group) = value.get("group") {
            let id = group.get("id")?.as_str()?.to_string();
            let count = usize::try_from(group.get("count")?.as_u64()?).ok()?;
            return Some(MemberListItem::Group(MemberListGroup { id, count }));
        }
        let member = parse_member_obj(value.get("member")?)?;
        Some(MemberListItem::Member(Box::new(member)))
    }

    let data = val.get("d").unwrap_or(val);
    let guild_id = data.get("guild_id")?.as_str()?.to_string();
    let id = data
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or("everyone")
        .to_string();
    let member_count = data
        .get("member_count")
        .and_then(|value| value.as_u64())
        .and_then(|count| usize::try_from(count).ok());
    let online_count = data
        .get("online_count")
        .and_then(|value| value.as_u64())
        .and_then(|count| usize::try_from(count).ok());

    let mut groups = Vec::new();
    if let Some(raw_groups) = data.get("groups").and_then(|value| value.as_array()) {
        for group in raw_groups {
            let id = group.get("id")?.as_str()?.to_string();
            let count = usize::try_from(group.get("count")?.as_u64()?).ok()?;
            groups.push(MemberListGroup { id, count });
        }
    }

    let raw_ops = data.get("ops")?.as_array()?;
    let mut ops = Vec::with_capacity(raw_ops.len());
    for operation in raw_ops {
        let op_type = operation.get("op")?.as_str()?;
        match op_type {
            "SYNC" => {
                let range = parse_range(operation.get("range"))?;
                let raw_items = operation.get("items")?.as_array()?;
                let mut items = Vec::with_capacity(raw_items.len());
                for item in raw_items {
                    items.push(parse_item(item)?);
                }
                ops.push(MemberListOp::Sync { range, items });
            }
            "INSERT" => {
                let index = parse_index(operation.get("index"))?;
                let item = parse_item(operation.get("item")?)?;
                ops.push(MemberListOp::Insert { index, item });
            }
            "UPDATE" => {
                let index = parse_index(operation.get("index"))?;
                let item = parse_item(operation.get("item")?)?;
                ops.push(MemberListOp::Update { index, item });
            }
            "DELETE" => {
                let index = parse_index(operation.get("index"))?;
                ops.push(MemberListOp::Delete { index });
            }
            "INVALIDATE" => {
                let range = parse_range(operation.get("range"))?;
                ops.push(MemberListOp::Invalidate { range });
            }
            _ => return None,
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
    let global_name = user
        .get("global_name")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let avatar = user
        .get("avatar")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let is_bot = user.get("bot").and_then(|v| v.as_bool()).unwrap_or(false);

    let avatar_decoration_data = user
        .get("avatar_decoration_data")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

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
        match p
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("offline")
        {
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
    let avatar_text = display
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string());
    let role_color = resolve_member_role_color(&member.roles, guild_roles);
    let role_group = resolve_member_role_group(&member.roles, guild_roles).unwrap_or_else(|| {
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

static AUTHOR_ROLES_CACHE: OnceLock<StdRwLock<HashMap<(String, String), Vec<String>>>> =
    OnceLock::new();
static AUTHOR_ROLE_COLOR_CACHE: OnceLock<StdRwLock<HashMap<(String, String), u32>>> =
    OnceLock::new();
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

/// Limpa todos os caches globais de roles ao trocar de conta/sessão.
pub fn clear_role_caches() {
    if let Ok(mut lock) = roles_cache().write() {
        lock.clear();
    }
    if let Ok(mut lock) = role_color_cache().write() {
        lock.clear();
    }
    if let Ok(mut lock) = guild_roles_cache().write() {
        lock.clear();
    }
}

pub fn get_cached_guild_roles(guild_id: &str) -> Vec<Role> {
    let lock = guild_roles_cache().read().ok();
    lock.and_then(|l| l.get(guild_id).cloned())
        .unwrap_or_default()
}

pub fn cache_author_roles(guild_id: &str, user_id: &str, roles: Vec<String>) {
    if let Ok(mut lock) = roles_cache().write() {
        lock.insert((guild_id.to_string(), user_id.to_string()), roles);
    }
}

pub fn get_cached_author_roles(guild_id: &str, user_id: &str) -> Option<Vec<String>> {
    let lock = roles_cache().read().ok()?;
    lock.get(&(guild_id.to_string(), user_id.to_string()))
        .cloned()
}

pub fn cache_author_role_color(guild_id: &str, user_id: &str, color: u32) {
    if let Ok(mut lock) = role_color_cache().write() {
        lock.insert((guild_id.to_string(), user_id.to_string()), color);
    }
}

pub fn get_cached_author_role_color(guild_id: &str, user_id: &str) -> Option<u32> {
    let lock = role_color_cache().read().ok()?;
    lock.get(&(guild_id.to_string(), user_id.to_string()))
        .copied()
}

static REQUESTED_OP8_MEMBERS: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashSet<(String, String)>>,
> = std::sync::OnceLock::new();

fn requested_op8_cache() -> &'static std::sync::RwLock<std::collections::HashSet<(String, String)>>
{
    REQUESTED_OP8_MEMBERS.get_or_init(|| std::sync::RwLock::new(std::collections::HashSet::new()))
}

pub fn is_op8_requested(guild_id: &str, user_id: &str) -> bool {
    let lock = requested_op8_cache().read().ok();
    lock.map(|l| l.contains(&(guild_id.to_string(), user_id.to_string())))
        .unwrap_or(false)
}

pub fn mark_op8_requested(guild_id: &str, user_id: &str) {
    if let Ok(mut lock) = requested_op8_cache().write() {
        lock.insert((guild_id.to_string(), user_id.to_string()));
    }
}

pub fn mark_op8_batch_requested(guild_id: &str, user_ids: &[String]) {
    if let Ok(mut lock) = requested_op8_cache().write() {
        for uid in user_ids {
            lock.insert((guild_id.to_string(), uid.clone()));
        }
    }
}

/// Reserva atomicamente apenas usuários ainda não pedidos/recebidos.
/// Em caso de falha de envio, chame `release_op8_claim` para permitir retry.
pub fn claim_missing_op8_users(guild_id: &str, user_ids: &[String]) -> Vec<String> {
    let Ok(mut lock) = requested_op8_cache().write() else {
        return Vec::new();
    };
    user_ids
        .iter()
        .filter(|user_id| lock.insert((guild_id.to_string(), (*user_id).clone())))
        .cloned()
        .collect()
}

pub fn release_op8_claim(guild_id: &str, user_ids: &[String]) {
    if let Ok(mut lock) = requested_op8_cache().write() {
        for user_id in user_ids {
            lock.remove(&(guild_id.to_string(), user_id.clone()));
        }
    }
}

pub fn clear_op8_requested() {
    if let Ok(mut lock) = requested_op8_cache().write() {
        lock.clear();
    }
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

/// Applies parsed `GuildMemberListUpdate` operations into the active member list.
/// Nunca fabrica slots "unloaded": um índice/range inválido invalida a lista
/// para que o Gateway envie um novo SYNC.
pub fn apply_member_list_update(
    existing_members: &mut Vec<MemberItemState>,
    update: &GuildMemberListUpdate,
    guild_roles: &[Role],
) {
    if update.ops.is_empty() {
        return;
    }

    for operation in &update.ops {
        match operation {
            MemberListOp::Sync { range, items } => {
                let [start, end] = *range;
                let Some(end_exclusive) = end.checked_add(1) else {
                    existing_members.clear();
                    continue;
                };
                if start > end {
                    existing_members.clear();
                    continue;
                }
                let range_len = end_exclusive - start;
                if items.len() > range_len {
                    existing_members.clear();
                    continue;
                }
                // Slots `unloaded` preservam o índice absoluto sem inventar
                // usuários; eles são marcadores explícitos de dados ausentes.
                if start > existing_members.len() {
                    existing_members
                        .extend((existing_members.len()..start).map(MemberItemState::unloaded));
                }

                let converted: Vec<MemberItemState> = items
                    .iter()
                    .map(|item| convert_list_item_to_state(item, &update.guild_id, guild_roles))
                    .collect();
                // Substitui integralmente o intervalo coberto, inclusive a
                // cauda antiga quando o novo SYNC traz menos itens.
                let replace_end = end_exclusive.min(existing_members.len()).max(start);
                existing_members.splice(start..replace_end, converted);
            }
            MemberListOp::Insert { index, item } => {
                if *index > existing_members.len() {
                    existing_members.clear();
                    continue;
                }
                let state = convert_list_item_to_state(item, &update.guild_id, guild_roles);
                existing_members.insert(*index, state);
            }
            MemberListOp::Update { index, item } => {
                if *index >= existing_members.len() {
                    existing_members.clear();
                    continue;
                }
                let state = convert_list_item_to_state(item, &update.guild_id, guild_roles);
                existing_members[*index] = state;
            }
            MemberListOp::Delete { index } => {
                if *index < existing_members.len() {
                    existing_members.remove(*index);
                }
            }
            MemberListOp::Invalidate { range } => {
                let [start, end] = *range;
                let Some(end_exclusive) = end.checked_add(1) else {
                    existing_members.clear();
                    continue;
                };
                if start > end || start >= existing_members.len() {
                    existing_members.clear();
                    continue;
                }
                let end = end_exclusive.min(existing_members.len());
                existing_members.drain(start..end);
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
            Role::new("r1", "Admin")
                .with_color(0xe91e63)
                .with_position(10),
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

    fn synthetic_member(id: &str) -> MemberListItem {
        MemberListItem::Member(Box::new(MemberListMember {
            id: id.to_string(),
            username: format!("user_{id}"),
            global_name: None,
            avatar: None,
            avatar_decoration_data: None,
            roles: Vec::new(),
            presence: PresenceStatus::Online,
            activity: None,
            is_bot: false,
        }))
    }

    #[test]
    fn test_member_list_parser_rejects_missing_or_inverted_ranges() {
        for operation in [
            json!({"op": "SYNC", "range": [10, 5], "items": []}),
            json!({"op": "SYNC", "items": []}),
            json!({"op": "DELETE"}),
            json!({"op": "INVALIDATE", "range": [5, 4]}),
        ] {
            let raw = json!({"guild_id": "guild_1", "ops": [operation]});
            assert!(parse_guild_member_list_update(&raw).is_none());
        }
    }

    #[test]
    fn test_sync_replaces_covered_tail_and_invalid_ranges_never_panic() {
        let mut members: Vec<MemberItemState> = (0..10)
            .map(|index| {
                convert_list_item_to_state(&synthetic_member(&index.to_string()), "guild_1", &[])
            })
            .collect();
        let update = GuildMemberListUpdate {
            guild_id: "guild_1".into(),
            id: "everyone".into(),
            member_count: None,
            online_count: None,
            groups: Vec::new(),
            ops: vec![MemberListOp::Sync {
                range: [0, 2],
                items: vec![synthetic_member("new")],
            }],
        };
        apply_member_list_update(&mut members, &update, &[]);
        assert_eq!(members.len(), 8);
        assert_eq!(members[0].id, "new");
        assert_eq!(members[1].id, "3");

        let invalid = GuildMemberListUpdate {
            guild_id: "guild_1".into(),
            id: "everyone".into(),
            member_count: None,
            online_count: None,
            groups: Vec::new(),
            ops: vec![MemberListOp::Invalidate {
                range: [0, usize::MAX],
            }],
        };
        apply_member_list_update(&mut members, &invalid, &[]);
        assert!(members.is_empty());
    }

    #[tokio::test]
    async fn test_coordinator_transitions_are_atomic_and_stale_viewport_is_dropped() {
        let coordinator = LazyGuildCoordinator::new();
        let first = coordinator.clone();
        let second = coordinator.clone();
        tokio::join!(
            first.focus_guild_and_channel("guild_a", "chan_a"),
            second.focus_guild_and_channel("guild_b", "chan_b")
        );

        let guild = coordinator.active_guild_id().await.unwrap();
        let channel = coordinator.active_channel_id().await.unwrap();
        assert!(
            (guild == "guild_a" && channel == "chan_a")
                || (guild == "guild_b" && channel == "chan_b")
        );
        assert_eq!(
            coordinator.active_ranges_for_channel(&channel).await,
            vec![[0, 99]]
        );
        assert!(
            coordinator
                .try_update_viewport_ranges("guild_old", "chan_old", vec![[0, 9]])
                .await
                .is_none()
        );

        let mut called = false;
        let result = coordinator
            .try_update_viewport_ranges_and_send("guild_old", "chan_old", vec![[0, 9]], |_| {
                called = true;
                std::future::ready::<anyhow::Result<()>>(Ok(()))
            })
            .await;
        assert!(result.is_none());
        assert!(!called);
    }

    #[test]
    fn test_op8_claim_is_atomic_and_release_allows_retry() {
        clear_op8_requested();
        let users = vec!["user_1".to_string(), "user_2".to_string()];
        assert_eq!(claim_missing_op8_users("guild_1", &users), users);
        assert!(claim_missing_op8_users("guild_1", &users).is_empty());
        release_op8_claim("guild_1", &users);
        assert_eq!(claim_missing_op8_users("guild_1", &users), users);
        clear_op8_requested();
    }

    #[test]
    fn test_clear_removes_role_caches_from_previous_account() {
        cache_guild_roles("guild_1", vec![Role::new("role_1", "Role")]);
        assert!(!get_cached_guild_roles("guild_1").is_empty());
        clear_role_caches();
        assert!(get_cached_guild_roles("guild_1").is_empty());
    }
}
