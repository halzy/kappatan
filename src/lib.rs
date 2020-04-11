mod error;

use error::KappaError;
use futures::prelude::*;
use template::markings::{Args, Opts, Template};
use template::Resolver;
use twitchchat::messages::Privmsg;

use std::collections::HashSet;

pub struct Bot<S>
where
    S: template::TemplateStore,
{
    templates: Resolver<S>,
    writer: twitchchat::Writer,
    control: twitchchat::Control,
    start: std::time::Instant,
}

impl<S> Bot<S>
where
    S: template::TemplateStore,
{
    pub fn new(mut control: twitchchat::Control, templates: Resolver<S>) -> Self {
        Self {
            templates,
            writer: control.writer().clone(),
            control,
            start: std::time::Instant::now(),
        }
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
        match data.chars().nth(0) {
            Some('!') => match self.handle_command(message).await {
                Err(err) => {
                    log::error!("Error handling message: {:?}", err);
                }
                _ => {}
            },
            _ => {}
        }
        /*
        match &*msg.data {
            "!hello" => {
                let resp = format!("hello {}!", msg.name);
                self.writer.privmsg(&msg.channel, &resp).await.unwrap();
            }
            "!uptime" => {
                let dur = std::time::Instant::now() - self.start;
                let resp = format!("I've been running for.. {:.2?}.", dur);
                self.writer.privmsg(&msg.channel, &resp).await.unwrap();
            }
            "!quit" => {
                // this'll stop the runner (causing its future to return Ok(Status::Canceled))
                self.control.stop();
                return false; // to stop the 'Bot'
            }
            _ => {}
        };
        */
        true // to keep the 'Bot' running
    }

    pub async fn handle_command(&mut self, message: &Privmsg<'_>) -> Result<(), KappaError>
    where
        S: template::TemplateStore,
    {
        let data = &message.data[1..];
        match data.split(" ").next() {
            Some(command) => {
                log::debug!("Command: {:?}", command);

                // Match templates that need 'generic'
                let template_string = match self.templates.resolve("templates", command) {
                    None => return Err(KappaError::TemplateMissing(command.to_string())),
                    Some(template_string) => template_string,
                };

                let mut keys: HashSet<&str> =
                    Template::find_keys(&template_string).map(|keys| keys.into_iter().collect())?;

                // If there are no keys, return the template string
                if keys.is_empty() {
                    self.writer
                        .privmsg(&message.channel, &template_string)
                        .await
                        .unwrap();
                    return Ok(());
                }

                let mut variables = Args::new();
                let name = message.display_name().unwrap_or_else(|| &message.name);
                if keys.remove("name") {
                    variables = variables.with("name", &name);
                }

                let template = Template::parse(&template_string, Opts::default())?;

                let variables = variables.build();
                let response = template.apply(&variables)?;
                self.writer.privmsg(&message.channel, &response).await?;
                Ok(())
            }
            _ => Err(KappaError::BadInput(data.to_string())),
        }
    }
}
