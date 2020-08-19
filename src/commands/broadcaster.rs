use super::command::Command;
use crate::error::Result;

impl super::Commands {
    /// !quit
    pub async fn quit(&mut self, _command: &Command<'_>) -> Result<()> {
        self.state.quit.clone().notify().await;
        Ok(())
    }

    /// !unset cmd
    pub async fn unset(&mut self, command: &Command<'_>, user_cmd: &str) -> Result<()> {
        let channel = command.channel;
        match sqlx::query_file!("sql/unset_template.sql", channel, user_cmd)
            .execute(&self.state.db)
            .await
        {
            Ok(_) => {
                let response = format!("'{}' has been unset.", user_cmd);
                self.send_response(command, response)
            }
            Err(err) => {
                log::warn!("Error saving template: {:?}", err);
                let response = format!("Was not able to unset '{}'", user_cmd);
                self.send_error(command, response)
            }
        }
    }

    /// !set cmd <template here>
    pub async fn set(
        &mut self,
        command: &Command<'_>,
        user_cmd: &str,
        template: Option<&str>,
    ) -> Result<()> {
        let template = match template {
            Some(template) => template,
            None => return self.send_help(command, "usage: !set <command> <template>"),
        };

        let channel = command.channel;
        // !set command template
        match sqlx::query_file!("sql/set_template.sql", channel, user_cmd, template)
            .execute(&self.state.db)
            .await
        {
            Ok(_) => {
                let response = format!("'{}' has been set to: {}", user_cmd, template);
                self.send_response(command, response)
            }
            Err(err) => {
                log::warn!("Error saving template: {:?}", err);
                let response = format!("Could not template for '{}'", user_cmd);
                self.send_error(command, response)
            }
        }
    }

    /// !give user amount
    pub async fn give(
        &mut self,
        command: &Command<'_>,
        user: &str,
        amount: Option<&str>,
    ) -> Result<()> {
        // Fetch numeric user ID from API
        const OUR_USER_ID: i64 = 75244893_i64;

        let value = match amount {
            Some(value) => value,
            None => return self.send_help(command, "usage: !give <user> <number>"),
        };

        let value: i64 = match value.parse() {
            Ok(value) => value,
            Err(err) => {
                log::warn!("Could not parse points: {}", err);
                let response = format!("Could not give '{}' to '{}'", value, user);
                return self.send_error(command, response);
            }
        };

        let channel = command.channel;
        match sqlx::query_file!("sql/give_points.sql", channel, OUR_USER_ID, value)
            .execute(&self.state.db)
            .await
        {
            Ok(_) => {
                let _ = self
                    .do_template(command, &"points", &user, Some(OUR_USER_ID as u64))
                    .await;
            }
            Err(err) => {
                log::warn!("Error giving points: {:?}", err);
                let response = format!("Could not give '{}' to '{}'", value, user);
                return self.send_error(command, response);
            }
        }

        Ok(())
    }
}
