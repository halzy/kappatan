use std::time::Instant;
use twitchchat::{
    runner::NotifyHandle,
    writer::{AsyncWriter, MpscWriter},
};

pub struct State {
    pub start: Instant,
    pub db: sqlx::SqlitePool,
    pub writer: AsyncWriter<MpscWriter>,
    pub quit: NotifyHandle,
}
