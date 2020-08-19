use twitchchat::{
    messages::AllCommands,
    runner::{AsyncRunner, Status},
    UserConfig,
};

use std::time::Instant;

mod error;
use error::{KappaError, Result};

mod commands;
use commands::{Command, Commands, State};

mod util;

pub struct Bot;

impl Bot {
    /// Run the bot to completion
    pub async fn run_to_completion(
        db: sqlx::SqlitePool,
        user_config: &UserConfig,
        channels: &[String],
    ) -> Result<bool> {
        // the flow of the crate is now rather simple.
        //
        // this is the part that is 'runtime' dependant.
        let connector = twitchchat::connector::TokioConnector::twitch();

        // you create a runner with the chosen connector and a &UserConfig.
        // this'll block until it connects and everything is set up
        let mut runner = AsyncRunner::connect(connector, user_config).await?;

        // and you can get your 'identity' here (username, user-id, etc).
        log::info!("connected! our name is: {}", runner.identity.username());

        // we'll be using this to store the state
        let state = State {
            // this lets us write
            writer: runner.writer(),
            // this lets us quit
            quit: runner.quit_handle(),
            start: Instant::now(),
            db,
        };

        for channel in channels {
            log::info!("joining '{}'", channel);
            // this 'join' is different from twitchchat::commands::join().
            // this blocks until you actually join the channel
            runner.join(channel).await?;
        }

        // and we can just simply loop until we're done
        Self::main_loop(runner, state).await
    }

    async fn main_loop(mut runner: AsyncRunner, state: State) -> Result<bool> {
        // the bots behavior is in a 'Commands' module
        let mut commands = Commands::new(state);

        loop {
            // this pulls the next message -- or connection/runner state
            match runner.next_message().await? {
                // you'll always get 'AllCommands' here. so we filter to 'Privmsg'
                Status::Message(AllCommands::Privmsg(pm)) => {
                    if let Some(cmd) = Command::parse(&pm) {
                        match commands.dispatch(&cmd).await {
                            Err(KappaError::BadInput(input)) => {
                                log::error!("invalid input from user: '{}'", input);
                            }
                            Err(err) => return Err(err),
                            _ => {}
                        }
                    }
                }
                // this happens when you notify on quit
                Status::Quit => return Ok(true),
                // this happens when the connection closes normally
                Status::Eof => {
                    log::info!("ending loop");
                    return Ok(false);
                }

                // ignore the rest of 'AllCommands'
                _ => continue,
            }
        }
    }
}
