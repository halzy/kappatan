mod error;
use error::KappaError;
type Result<T> = ::std::result::Result<T, KappaError>;

use markings::{Args, Opts, Template};

use twitchchat::PrivmsgExt as _;
use twitchchat::{
    messages::{AllCommands, Privmsg},
    runner::{AsyncRunner, NotifyHandle, Status},
    writer::{AsyncWriter, MpscWriter},
    UserConfig,
};

use std::{collections::HashSet, time::Instant};

#[derive(Debug)]
struct TemplateQuery {
    template: String,
}

#[derive(Debug)]
struct Points {
    points: i32,
}

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

struct Commands {
    state: State,
}

impl Commands {
    const fn new(state: State) -> Self {
        Self { state }
    }

    async fn dispatch(&mut self, command: &Command<'_>) -> Result<()> {
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

    async fn list_commands(&mut self, command: &Command<'_>) -> Result<()> {
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

    async fn quit(&mut self, _command: &Command<'_>) -> Result<()> {
        self.state.quit.clone().notify().await;
        Ok(())
    }

    async fn unset(&mut self, command: &Command<'_>, user_cmd: &str) -> Result<()> {
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

    async fn set(
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

    async fn give(
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

    async fn do_template(
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

fn split_user_cmd(data: &str) -> Option<(&str, Option<&str>)> {
    let mut iter = data
        .splitn(2, ' ')
        .filter_map(|s| Some(s.trim()).filter(|c| !c.is_empty()));
    Some((iter.next()?, iter.next()))
}

struct State {
    start: Instant,
    db: sqlx::SqlitePool,
    writer: AsyncWriter<MpscWriter>,
    quit: NotifyHandle,
}

struct Command<'a> {
    cmd: &'a str,
    args: Option<&'a str>,
    msg: &'a Privmsg<'a>,
    channel: &'a str,
}

impl<'a> Command<'a> {
    fn parse(msg: &'a Privmsg<'a>) -> Option<Command<'a>> {
        const TRIGGER: &str = "!";

        let data = msg.data();
        if !data.starts_with(TRIGGER) || data.len() == TRIGGER.len() {
            return None;
        }

        let mut iter = data.splitn(2, ' ');
        let (head, tail) = (iter.next()?, iter.next());

        Some(Command {
            cmd: &head[TRIGGER.len()..],
            args: tail,
            msg,
            channel: &msg.channel()[1..],
        })
    }
}
