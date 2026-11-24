use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Discord epoch in milliseconds (2015-01-01T00:00:00.000Z).
pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// Discord Snowflake identifier.
///
/// Stores the raw u64 value. Chronological ordering is O(1) because the
/// timestamp occupies the highest 42 bits of the snowflake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Snowflake(pub u64);

impl Snowflake {
    /// Creates a snowflake from its raw u64 value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw u64 value.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Milliseconds since Unix epoch encoded in this snowflake.
    pub const fn timestamp_millis(self) -> u64 {
        (self.0 >> 22) + DISCORD_EPOCH_MS
    }

    /// Returns the creation timestamp as a UTC datetime.
    pub fn created_at(self) -> DateTime<Utc> {
        let ms = self.timestamp_millis() as i64;
        Utc.timestamp_millis_opt(ms)
            .single()
            .unwrap_or_else(|| Utc.timestamp_nanos(0))
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for Snowflake {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Snowflake> for u64 {
    fn from(value: Snowflake) -> Self {
        value.0
    }
}

impl FromStr for Snowflake {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        Ok(Self(trimmed.parse::<u64>()?))
    }
}

impl Serialize for Snowflake {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnowflakeVisitor;

        impl<'de> serde::de::Visitor<'de> for SnowflakeVisitor {
            type Value = Snowflake;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a snowflake as a JSON string or unsigned integer")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value
                    .trim()
                    .parse::<u64>()
                    .map(Snowflake)
                    .map_err(|e| E::custom(format!("invalid snowflake string: {e}")))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Snowflake(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value < 0 {
                    return Err(E::custom("snowflake cannot be negative"));
                }
                Ok(Snowflake(value as u64))
            }
        }

        deserializer.deserialize_any(SnowflakeVisitor)
    }
}
