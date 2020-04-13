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
            (Some(command), None) => {
                log::debug!("Command: {:?}", command);

                // Match templates that need 'generic'
                let channel = &message.channel[1..];
                let template_query =
                    sqlx::query_file_as!(TemplateQuery, "sql/get_template.sql", channel, command)
                        .fetch_one(&self.db)
                        .await?;

                let mut keys: HashSet<&str> = Template::find_keys(&template_query.template)
                    .map(|keys| keys.into_iter().collect())?;

                // If there are no keys, return the template string
                if keys.is_empty() {
                    self.writer
                        .privmsg(&message.channel, &template_query.template)
                        .await
                        .unwrap();
                    return Ok(());
                }

                let mut variables = Args::new();
                if keys.remove("name") {
                    let name = message.display_name().unwrap_or_else(|| &message.name);
                    variables = variables.with("name", &name);
                }

                if keys.remove("uptime") {
                    let uptime = std::time::Instant::now() - self.start;
                    let uptime = as_readable_time(&uptime);
                    variables = variables.with("uptime", &uptime);
                }

                let template = Template::parse(&template_query.template, Opts::default())?;

                let response = template.apply(&variables)?;
                self.writer.privmsg(&message.channel, &response).await?;
                Ok(())
            }
            _ => Err(KappaError::BadInput(data.to_string())),
        }
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
