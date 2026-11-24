//! Discord RS - Chat Stream and Editor Module
//!
//! Submodules:
//! - `blocked_group`: Consecutive blocked-author messages with expand/collapse state
//! - `chat_view`: Real-time reactive ChatView orchestrating message stream and editor
//! - `markdown`: Discord Markdown parser & native GPUI renderer
//! - `message_list`: Cozy style message stream, 5-minute grouping, avatars, timestamps, hover bar
//! - `reactions`: Reaction counters, active indicators, and add-reaction controls
//! - `editor`: Bottom input textarea, active reply banner, rich buttons, typing indicator
//! - `polls`: Native poll model, parsing, percentages, and GPUI card

pub mod blocked_group;
pub mod chat_view;
pub mod editor;
pub mod markdown;
pub mod media_picker;
pub mod message_list;
pub mod polls;
pub mod reactions;
pub mod search;
pub mod system_messages;

pub use blocked_group::*;
pub use chat_view::*;
pub use editor::*;
pub use markdown::*;
pub use media_picker::*;
pub use message_list::*;
pub use polls::*;
pub use reactions::*;
pub use search::*;
pub use system_messages::*;


