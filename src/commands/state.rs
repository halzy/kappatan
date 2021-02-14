use std::time::Instant;
use twitchchat::{
    runner::NotifyHandle,
    writer::{AsyncWriter, MpscWriter},
};

/// State of the bot
pub struct State {
    /// our start time
    pub start: Instant,
    /// db pool
    pub db: sqlx::SqlitePool,
    /// something we can write to
    pub writer: AsyncWriter<MpscWriter>,
    /// handle to signal a quit
    pub quit: NotifyHandle,
}
