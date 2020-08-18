mod error;

use error::KappaError;
use futures::prelude::*;
use markings::{Args, Opts, Template};
use twitchchat::messages::Privmsg;

use std::collections::HashSet;

#[derive(Debug)]
struct TemplateQuery {
    template: String,
}

#[derive(Debug)]
struct Points {
    points: i32,
}

pub struct Bot {
    db: sqlx::SqlitePool,
    writer: twitchchat::Writer,
    control: twitchchat::Control,
    start: std::time::Instant,
}

impl Bot {
    pub fn create(
        mut control: twitchchat::Control,
        db: sqlx::SqlitePool,
    ) -> Result<Self, KappaError> {
        Ok(Self {
            db,
            writer: control.writer().clone(),
            control,
            start: std::time::Instant::now(),
        })
    }

    pub async fn run(
        mut self,
        dispatcher: twitchchat::Dispatcher,
        channel: impl twitchchat::IntoChannel,
    ) {
        // subscribe to the events we're interested in
        let mut events = dispatcher.subscribe::<twitchchat::events::Privmsg>();

        // and wait for a specific event (blocks the current task)
        let ready = dispatcher
            .wait_for::<twitchchat::events::IrcReady>()
            .await
            .unwrap();
        log::info!("connected! our name is: {}", ready.nickname);

        // and then join a channel
        log::info!("joining our channel");
        self.writer.join(channel).await.unwrap();

        // and then our 'main loop'
        while let Some(message) = events.next().await {
            if !self.handle(&*message).await {
                return;
            }
        }
    }

    async fn handle(&mut self, message: &twitchchat::messages::Privmsg<'_>) -> bool {
        let data = &message.data;
        match data.chars().next() {
            Some('!') => match self.handle_command(message).await {
                Err(err) => {
                    log::error!("Error handling message: {:?}", err);
                }
                _ => {}
            },
            _ => {}
        }
        true // to keep the 'Bot' running
    }

    pub async fn handle_command(&mut self, message: &Privmsg<'_>) -> Result<(), KappaError> {
        // Strip the ! from the beginning
        let data = &message.data[1..];
        let mut iter = data.splitn(3, ' ').filter(|s| !s.is_empty());
        match (iter.next(), iter.next()) {
            (Some("quit"), None) if message.is_broadcaster() => {
                self.control.stop();
                Ok(())
            }
            (Some("unset"), Some(command)) if message.is_broadcaster() => {
                let channel = &message.channel[1..];
                match sqlx::query_file!("sql/unset_template.sql", channel, command)
                    .execute(&self.db)
                    .await
                {
                    Ok(_) => {
                        let response = format!("'{}' has been unset.", command);
                        self.writer.privmsg(&message.channel, response).await?;
                    }
                    Err(err) => {
                        log::warn!("Error saving template: {:?}", err);
                        let response = format!("Was not able to unset '{}'", command);
                        self.writer.privmsg(&message.channel, response).await?;
                    }
                }
                Ok(())
            }
            (Some("set"), Some(command)) if message.is_broadcaster() => {
                match iter.next() {
                    Some(template) => {
                        let channel = &message.channel[1..];
                        // !set command template
                        match sqlx::query_file!("sql/set_template.sql", channel, command, template)
                            .execute(&self.db)
                            .await
                        {
                            Ok(_) => {
                                let response =
                                    format!("'{}' has been set to: {}", command, template);
                                self.writer.privmsg(&message.channel, response).await?;
                            }
                            Err(err) => {
                                log::warn!("Error saving template: {:?}", err);
                                let response = format!("Could not template for '{}'", command);
                                self.writer.privmsg(&message.channel, response).await?;
                            }
                        }
                    }
                    None => {
                        let response = "usage: !set <command> <template>";
                        self.writer.privmsg(&message.channel, response).await?;
                    }
                }
                Ok(())
            }
            (Some("give"), Some(user)) if message.is_broadcaster() => {
                match iter.next() {
                    Some(value) => {
                        // Fetch numeric user ID from API
                        let user_id = 75244893_i64;
                        let value: i64 = match value.parse() {
                            Ok(value) => value,
                            Err(err) => {
                                log::warn!("Could not parse points: {:0}", err);
                                let response = format!("Could not give '{}' to '{}'", value, user);
                                self.writer.privmsg(&message.channel, response).await?;
                                return Ok(());
                            }
                        };

                        let channel = &*message.channel;
                        match sqlx::query_file!("sql/give_points.sql", channel, user_id, value)
                            .execute(&self.db)
                            .await
                        {
                            Ok(_) => {
                                let _ = self
                                    .do_template(
                                        &message.channel,
                                        &"points",
                                        &user,
                                        Some(user_id as u64),
                                    )
                                    .await;
                            }
                            Err(err) => {
                                log::warn!("Error giving points: {:?}", err);
                                let response = format!("Could not give '{}' to '{}'", value, user);
                                self.writer.privmsg(&message.channel, response).await?;
                            }
                        }
                    }
                    None => {
                        let response = "usage: !give <user> <number>";
                        self.writer.privmsg(&message.channel, response).await?;
                    }
                }

                Ok(())
            }
            (Some("commands"), None) => {
                let channel = &message.channel[1..];
                match sqlx::query_file!("sql/list_commands.sql", channel)
                    .fetch_all(&self.db)
                    .await
                {
                    Ok(value) => {
                        let commands = value
                            .iter()
                            .map(|record| record.command.as_ref())
                            .collect::<Vec<&str>>()
                            .join(", !");

                        self.writer
                            .privmsg(
                                &message.channel,
                                format!("Currently available commands: !{}", commands),
                            )
                            .await?;
                        Ok(())
                    }
                    Err(err) => {
                        log::warn!("Error fetching the list of commands: {:?}", err);
                        self.writer
                            .privmsg(&message.channel, "Could not fetch the list of commands")
                            .await?;
                        Ok(())
                    }
                }
            }
            (Some(command), None) => {
                self.do_template(&message.channel, &command, &message.name, message.user_id())
                    .await?;

                Ok(())
            }
            _ => Err(KappaError::BadInput(data.to_string())),
        }
    }

    async fn do_template(
        &mut self,
        channel: &str,
        command: &str,
        user_name: &str,
        user_id: Option<u64>,
    ) -> Result<(), KappaError> {
        log::debug!("Command: {:?}", command);

        // Match templates that need 'generic'
        let template_channel = &channel[1..];
        let template_query = sqlx::query_file_as!(
            TemplateQuery,
            "sql/get_template.sql",
            template_channel,
            command
        )
        .fetch_one(&self.db)
        .await?;

        let mut keys: HashSet<&str> =
            Template::find_keys(&template_query.template).map(|keys| keys.into_iter().collect())?;

        // If there are no keys, return the template string
        if keys.is_empty() {
            self.writer
                .privmsg(&channel, &template_query.template)
                .await
                .unwrap();
            return Ok(());
        }

        let mut variables = Args::new();
        if keys.remove("name") {
            variables = variables.with("name", &user_name);
        }

        if keys.remove("botuptime") {
            let uptime = std::time::Instant::now() - self.start;
            let uptime = as_readable_time(&uptime);
            variables = variables.with("botuptime", &uptime);
        }

        if keys.remove("points") && user_id.is_some() {
            let user_id = user_id.unwrap() as i64;
            let points = sqlx::query_file_as!(Points, "sql/get_points.sql", channel, user_id)
                .fetch_one(&self.db)
                .await?;
            variables = variables.with("points", &points.points);
        }

        let template = Template::parse(&template_query.template, Opts::default())?;

        let response = template.apply(&variables)?;
        self.writer.privmsg(&channel, &response).await?;
        Ok(())
    }
}

fn as_readable_time(dur: &std::time::Duration) -> String {
    const TABLE: [(&str, u64); 4] = [
        ("days", 86400),
        ("hours", 3600),
        ("minutes", 60),
        ("seconds", 1),
    ];

    fn pluralize(s: &&str, n: u64) -> String {
        format!("{} {}", n, if n > 1 { s } else { &s[..s.len() - 1] })
    }

    let mut time = vec![];
    let mut secs = dur.as_secs();
    for (name, d) in &TABLE {
        let div = secs / d;
        if div > 0 {
            time.push(pluralize(name, div));
            secs -= d * div;
        }
    }

    let len = time.len();
    if len > 1 {
        if len > 2 {
            for e in &mut time.iter_mut().take(len - 2) {
                e.push_str(",")
            }
        }
        time.insert(len - 1, "and".into());
    }
    time.join(" ")
}
