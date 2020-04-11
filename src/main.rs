use kappatan::Bot;

use std::env;
use std::path::PathBuf;

use futures::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load the config
    dotenv::dotenv()?;

    alto_logger::init(
        alto_logger::Style::MultiLine,
        alto_logger::ColorConfig::default(),
    )
    .unwrap();

    log::trace!("Loading environment");
    let nick = env::var("TWITCH_NICK")?;
    let oauth = env::var("TWITCH_OAUTH")?;
    let channel = env::var("TWITCH_CHANNEL")?;
    let templates: PathBuf = env::var("TEMPLATES_FILE")?.parse()?;

    // Template resolver here
    let template_file = template::FileStore::new(templates, template::load_toml)?;
    let resolver = template::Resolver::new(template_file)?;

    // make a dispatcher (this is how you 'subscribe' to events)
    // this is clonable, so you can send it to other tasks/threasd
    let dispatcher = twitchchat::Dispatcher::new();

    // make a new runner
    // control allows you to stop the runner, and gives you access to an async. encoder (writer)
    let (runner, control) =
        twitchchat::Runner::new(dispatcher.clone(), twitchchat::RateLimit::default());

    let bot = Bot::new(control, resolver);
    let bot = bot.run(dispatcher, channel);

    // connect via TCP with TLS with this nick and oauth
    let conn = twitchchat::connect_easy_tls(&nick, &oauth).await.unwrap();

    let done = runner.run(conn);

    tokio::select! {
        _ = bot => { eprintln!("done running the bot") }
        status = done => {
            match status {
                Ok(twitchchat::Status::Timeout) => {
                    log::warn!("Connection to server timed out!");
                }
                Ok(twitchchat::Status::Eof) => {
                    log::warn!("Connection closed. Shutting down.");
                }
                Ok(twitchchat::Status::Canceled) => {
                    log::warn!("Shutting down.");
                }
                Err(err) => {
                    log::warn!("Error, shutting down. {:?}", err);
                }
            }
        }
    }

    Ok(())
}
