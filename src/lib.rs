use twitchchat::{
    messages::AllCommands,
    runner::{AsyncRunner, Status},
    UserConfig,
};

use std::time::Instant;

mod error;
use error::Result;

mod commands;
use commands::{Command, Commands, State};

mod util;

pub struct Bot;

impl Bot {
    pub async fn run_to_completion(
        db: sqlx::SqlitePool,
        user_config: &UserConfig,
        channels: &[String],
    ) -> Result<bool> {
        let connector = twitchchat::connector::TokioConnector::twitch();
        let mut runner = AsyncRunner::connect(connector, user_config).await?;

        log::info!("connected! our name is: {}", runner.identity.username());

        let state = State {
            writer: runner.writer(),
            quit: runner.quit_handle(),
            start: Instant::now(),
            db,
        };

        for channel in channels {
            log::info!("joining '{}'", channel);
            runner.join(channel).await?;
        }

        Self::main_loop(runner, state).await
    }

    async fn main_loop(mut runner: AsyncRunner, state: State) -> Result<bool> {
        let mut commands = Commands::new(state);

        loop {
            match runner.next_message().await? {
                Status::Message(AllCommands::Privmsg(pm)) => {
                    if let Some(cmd) = Command::parse(&pm) {
                        commands.dispatch(&cmd).await?;
                    }
                }
                Status::Quit => return Ok(true),
                Status::Eof => {
                    log::info!("ending loop");
                    return Ok(false);
                }
                _ => continue,
            }
        }
    }
}
