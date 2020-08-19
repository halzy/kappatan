use crate::error::{KappaError, Result};

use twitchchat::PrivmsgExt;

mod broadcaster;
mod user;

mod command;
pub use command::Command;

mod state;
pub use state::State;

fn split_user_cmd(data: &str) -> Option<(&str, Option<&str>)> {
    let mut iter = data
        .splitn(2, ' ')
        .filter_map(|s| Some(s.trim()).filter(|c| !c.is_empty()));
    Some((iter.next()?, iter.next()))
}

pub struct Commands {
    state: State,
}

impl Commands {
    pub const fn new(state: State) -> Self {
        Self { state }
    }

    // entry point for this
    pub async fn dispatch(&mut self, command: &Command<'_>) -> Result<()> {
        // broadcaster commands
        if command.msg.is_broadcaster() {
            self.elevated_dispatch(command).await?;
        }
        self.normal_dispatch(command).await
    }

    // broadcaster commands
    async fn elevated_dispatch(&mut self, command: &Command<'_>) -> Result<()> {
        match (command.cmd, command.args) {
            ("quit", None) => return self.quit(command).await,
            ("unset", Some(user_cmd)) => return self.unset(command, user_cmd).await,
            ("set", Some(user_cmd)) => {
                // split 'user_cmd' into 'name' 'template here'?
                let (user_cmd, template) = split_user_cmd(user_cmd).unwrap();
                return self.set(command, user_cmd, template).await;
            }
            ("give", Some(user_cmd)) => {
                // split 'user_cmd' into 'name' 'points'?
                let (user_cmd, template) = split_user_cmd(user_cmd).unwrap();
                return self.give(command, user_cmd, template).await;
            }
            _ => {}
        }
        Ok(())
    }

    // all user commands
    async fn normal_dispatch(&mut self, command: &Command<'_>) -> Result<()> {
        match (command.cmd, command.args) {
            ("commands", None) => self.list_commands(command).await,
            (user_cmd, None) => {
                let (name, user_id) = (command.msg.name(), command.msg.user_id());
                self.do_template(command, user_cmd, name, user_id).await
            }
            _ => Err(KappaError::BadInput(command.msg.data().to_string())),
        }
    }

    fn send_response(&mut self, command: &Command<'_>, response: impl AsRef<str>) -> Result<()> {
        command.msg.say(&mut self.state.writer, response.as_ref())?;
        Ok(())
    }

    fn send_help(&mut self, command: &Command<'_>, help: impl AsRef<str>) -> Result<()> {
        command.msg.say(&mut self.state.writer, help.as_ref())?;
        Ok(())
    }

    fn send_error(&mut self, command: &Command<'_>, error: impl AsRef<str>) -> Result<()> {
        command.msg.say(&mut self.state.writer, error.as_ref())?;
        Ok(())
    }
}
