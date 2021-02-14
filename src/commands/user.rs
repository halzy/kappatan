use super::command::Command;
use crate::{error::Result, util::as_readable_time};

use markings::{Args, Opts, Template};
use std::collections::HashSet;

#[derive(Debug)]
struct TemplateQuery {
    template: String,
}

#[derive(Debug)]
struct Points {
    points: i32,
}

impl super::Commands {
    /// '!commands'
    pub async fn list_commands(&mut self, command: &Command<'_>) -> Result<()> {
        let channel = command.channel;
        let value = match sqlx::query_file!("sql/list_commands.sql", channel)
            .fetch_all(&self.state.db)
            .await
        {
            Ok(value) => value,
            Err(err) => {
                log::warn!("Error fetching the list of commands: {:?}", err);
                return self.send_error(command, "Could not fetch the list of commands");
            }
        };

        let commands = value
            .iter()
            .map(|record| record.command.as_ref())
            .collect::<Vec<&str>>()
            .join(", !");

        self.send_response(
            command,
            format!("Currently available commands: !{}", commands),
        )
    }

    /// !<user_cmd>
    pub async fn do_template(
        &mut self,
        command: &Command<'_>,
        user_command: &str,
        user_name: &str,
        user_id: Option<u64>,
    ) -> Result<()> {
        log::debug!("Command: {:?}", user_command);

        // Match templates that need 'generic'
        let template_channel = command.channel;
        let template_query = sqlx::query_file_as!(
            TemplateQuery,
            "sql/get_template.sql",
            template_channel,
            user_command
        )
        .fetch_one(&self.state.db)
        .await?;

        let mut keys: HashSet<&str> = Template::find_keys(&template_query.template) //
            .map(|keys| keys.into_iter().collect())?;

        // If there are no keys, return the template string
        if keys.is_empty() {
            return self.send_response(command, template_query.template);
        }

        let mut variables = Args::new();
        if keys.remove("name") {
            variables = variables.with("name", &user_name);
        }

        if keys.remove("botuptime") {
            let uptime = as_readable_time(&self.state.start.elapsed());
            variables = variables.with("botuptime", &uptime);
        }

        if keys.remove("points") && user_id.is_some() {
            let user_id = user_id.unwrap() as i64;
            let points =
                sqlx::query_file_as!(Points, "sql/get_points.sql", template_channel, user_id)
                    .fetch_one(&self.state.db)
                    .await?;
            variables = variables.with("points", &points.points);
        }

        let template = Template::parse(&template_query.template, Opts::default())?;
        let response = template.apply(&variables)?;
        self.send_response(command, response)
    }
}
