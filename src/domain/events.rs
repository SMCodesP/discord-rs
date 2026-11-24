use serde::{Deserialize, Serialize};

/// Privacy level for a Stage instance.
/// Discord: https://discord.com/developers/docs/resources/stage-instance#stage-instance-object-privacy-level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum StageInstancePrivacyLevel {
    /// Public stage instance (deprecated by Discord).
    Public = 1,
    /// Stage instance visible only to guild members.
    GuildOnly = 2,
}

impl TryFrom<u8> for StageInstancePrivacyLevel {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Public),
            2 => Ok(Self::GuildOnly),
            other => Err(format!("Unknown StageInstancePrivacyLevel: {other}")),
        }
    }
}

impl From<StageInstancePrivacyLevel> for u8 {
    fn from(value: StageInstancePrivacyLevel) -> Self {
        value as u8
    }
}

/// A live Stage instance associated with a stage channel.
/// Discord: https://discord.com/developers/docs/resources/stage-instance#stage-instance-object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageInstance {
    /// Snowflake ID of this stage instance.
    pub id: String,
    /// Guild Snowflake ID of the associated stage channel.
    pub guild_id: String,
    /// Channel Snowflake ID of the associated stage channel.
    pub channel_id: String,
    /// Topic of the stage instance (1-120 characters).
    pub topic: String,
    /// Privacy level of the stage instance.
    pub privacy_level: StageInstancePrivacyLevel,
    /// Whether Stage Discovery is disabled (deprecated by Discord).
    #[serde(default)]
    pub discoverable_disabled: bool,
    /// The ID of the scheduled event for this stage instance, if any.
    #[serde(default)]
    pub guild_scheduled_event_id: Option<String>,
}

/// Privacy level for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-object-guild-scheduled-event-privacy-level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum GuildScheduledEventPrivacyLevel {
    /// Scheduled event is only accessible to guild members.
    GuildOnly = 2,
}

impl TryFrom<u8> for GuildScheduledEventPrivacyLevel {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            2 => Ok(Self::GuildOnly),
            other => Err(format!("Unknown GuildScheduledEventPrivacyLevel: {other}")),
        }
    }
}

impl From<GuildScheduledEventPrivacyLevel> for u8 {
    fn from(value: GuildScheduledEventPrivacyLevel) -> Self {
        value as u8
    }
}

/// Status of a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-object-guild-scheduled-event-status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum GuildScheduledEventStatus {
    Scheduled = 1,
    Active = 2,
    Completed = 3,
    Canceled = 4,
}

impl TryFrom<u8> for GuildScheduledEventStatus {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Scheduled),
            2 => Ok(Self::Active),
            3 => Ok(Self::Completed),
            4 => Ok(Self::Canceled),
            other => Err(format!("Unknown GuildScheduledEventStatus: {other}")),
        }
    }
}

impl From<GuildScheduledEventStatus> for u8 {
    fn from(value: GuildScheduledEventStatus) -> Self {
        value as u8
    }
}

/// Entity type for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-object-guild-scheduled-event-entity-types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum GuildScheduledEventEntityType {
    StageInstance = 1,
    Voice = 2,
    External = 3,
}

impl TryFrom<u8> for GuildScheduledEventEntityType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::StageInstance),
            2 => Ok(Self::Voice),
            3 => Ok(Self::External),
            other => Err(format!("Unknown GuildScheduledEventEntityType: {other}")),
        }
    }
}

impl From<GuildScheduledEventEntityType> for u8 {
    fn from(value: GuildScheduledEventEntityType) -> Self {
        value as u8
    }
}

/// Additional metadata for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-object-guild-scheduled-event-entity-metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GuildScheduledEventEntityMetadata {
    /// Location of the event (required for EXTERNAL events).
    #[serde(default)]
    pub location: Option<String>,
}

/// Recurrence rule frequency for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-recurrence-rule-object-guild-scheduled-event-recurrence-rule-frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum GuildScheduledEventRecurrenceRuleFrequency {
    Yearly = 0,
    Monthly = 1,
    Weekly = 2,
    Daily = 3,
}

impl TryFrom<u8> for GuildScheduledEventRecurrenceRuleFrequency {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Yearly),
            1 => Ok(Self::Monthly),
            2 => Ok(Self::Weekly),
            3 => Ok(Self::Daily),
            other => Err(format!(
                "Unknown GuildScheduledEventRecurrenceRuleFrequency: {other}"
            )),
        }
    }
}

impl From<GuildScheduledEventRecurrenceRuleFrequency> for u8 {
    fn from(value: GuildScheduledEventRecurrenceRuleFrequency) -> Self {
        value as u8
    }
}

/// Recurrence rule weekday for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-recurrence-rule-object-guild-scheduled-event-recurrence-rule-weekday
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum GuildScheduledEventRecurrenceRuleWeekday {
    Monday = 0,
    Tuesday = 1,
    Wednesday = 2,
    Thursday = 3,
    Friday = 4,
    Saturday = 5,
    Sunday = 6,
}

impl TryFrom<u8> for GuildScheduledEventRecurrenceRuleWeekday {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Monday),
            1 => Ok(Self::Tuesday),
            2 => Ok(Self::Wednesday),
            3 => Ok(Self::Thursday),
            4 => Ok(Self::Friday),
            5 => Ok(Self::Saturday),
            6 => Ok(Self::Sunday),
            other => Err(format!(
                "Unknown GuildScheduledEventRecurrenceRuleWeekday: {other}"
            )),
        }
    }
}

impl From<GuildScheduledEventRecurrenceRuleWeekday> for u8 {
    fn from(value: GuildScheduledEventRecurrenceRuleWeekday) -> Self {
        value as u8
    }
}

/// Recurrence rule month for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-recurrence-rule-object-guild-scheduled-event-recurrence-rule-month
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum GuildScheduledEventRecurrenceRuleMonth {
    January = 1,
    February = 2,
    March = 3,
    April = 4,
    May = 5,
    June = 6,
    July = 7,
    August = 8,
    September = 9,
    October = 10,
    November = 11,
    December = 12,
}

impl TryFrom<u8> for GuildScheduledEventRecurrenceRuleMonth {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::January),
            2 => Ok(Self::February),
            3 => Ok(Self::March),
            4 => Ok(Self::April),
            5 => Ok(Self::May),
            6 => Ok(Self::June),
            7 => Ok(Self::July),
            8 => Ok(Self::August),
            9 => Ok(Self::September),
            10 => Ok(Self::October),
            11 => Ok(Self::November),
            12 => Ok(Self::December),
            other => Err(format!(
                "Unknown GuildScheduledEventRecurrenceRuleMonth: {other}"
            )),
        }
    }
}

impl From<GuildScheduledEventRecurrenceRuleMonth> for u8 {
    fn from(value: GuildScheduledEventRecurrenceRuleMonth) -> Self {
        value as u8
    }
}

/// Nth weekday rule for a guild scheduled event recurrence.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-recurrence-rule-object-guild-scheduled-event-recurrence-rule-nweekday-structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildScheduledEventRecurrenceRuleNWeekday {
    /// The index for the rule representation (1-5).
    pub n: u8,
    /// The day within a week the event recurs on.
    pub day: GuildScheduledEventRecurrenceRuleWeekday,
}

/// Recurrence rule for a guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-recurrence-rule-object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildScheduledEventRecurrenceRule {
    /// Starting time of the recurrence interval (ISO8601 timestamp).
    pub start: String,
    /// Ending time of the recurrence interval (ISO8601 timestamp).
    #[serde(default)]
    pub end: Option<String>,
    /// How often the event occurs.
    pub frequency: GuildScheduledEventRecurrenceRuleFrequency,
    /// The spacing between the events, defined by frequency.
    pub interval: u32,
    /// Set of specific days within a week for the event to recur on.
    #[serde(default)]
    pub by_weekday: Option<Vec<GuildScheduledEventRecurrenceRuleWeekday>>,
    /// List of specific days within a specific week to recur on.
    #[serde(default)]
    pub by_n_weekday: Option<Vec<GuildScheduledEventRecurrenceRuleNWeekday>>,
    /// Set of specific months to recur on.
    #[serde(default)]
    pub by_month: Option<Vec<GuildScheduledEventRecurrenceRuleMonth>>,
    /// Set of specific dates within a month to recur on.
    #[serde(default)]
    pub by_month_day: Option<Vec<u8>>,
    /// Set of days within a year to recur on (1-364).
    #[serde(default)]
    pub by_year_day: Option<Vec<u16>>,
    /// The total amount of times the event is allowed to recur before stopping.
    #[serde(default)]
    pub count: Option<u32>,
}

/// A guild scheduled event.
/// Discord: https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildScheduledEvent {
    /// The id of the scheduled event.
    pub id: String,
    /// The guild id which the scheduled event belongs to.
    pub guild_id: String,
    /// The channel id in which the scheduled event will be hosted, or null if external.
    #[serde(default)]
    pub channel_id: Option<String>,
    /// The id of the user that created the scheduled event.
    #[serde(default)]
    pub creator_id: Option<String>,
    /// The name of the scheduled event (1-100 characters).
    pub name: String,
    /// The description of the scheduled event (1-1000 characters).
    #[serde(default)]
    pub description: Option<String>,
    /// The time the scheduled event will start (ISO8601 timestamp).
    pub scheduled_start_time: String,
    /// The time the scheduled event will end, or null if not scheduled to end.
    #[serde(default)]
    pub scheduled_end_time: Option<String>,
    /// The privacy level of the scheduled event.
    pub privacy_level: GuildScheduledEventPrivacyLevel,
    /// The status of the scheduled event.
    pub status: GuildScheduledEventStatus,
    /// The type of the scheduled event.
    pub entity_type: GuildScheduledEventEntityType,
    /// The id of an entity associated with a guild scheduled event.
    #[serde(default)]
    pub entity_id: Option<String>,
    /// Additional metadata for the guild scheduled event.
    #[serde(default)]
    pub entity_metadata: Option<GuildScheduledEventEntityMetadata>,
    /// The number of users subscribed to the scheduled event.
    #[serde(default)]
    pub user_count: Option<u32>,
    /// The cover image hash of the scheduled event.
    #[serde(default)]
    pub image: Option<String>,
    /// The definition for how often this event should recur.
    #[serde(default)]
    pub recurrence_rule: Option<GuildScheduledEventRecurrenceRule>,
}
