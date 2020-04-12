use kappatan::Bot;

use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load the config
    dotenv::from_filename(".env.production")?;

    alto_logger::init(
        alto_logger::Style::MultiLine,
        alto_logger::ColorConfig::default(),
    )
    .unwrap();

    let nick = env::var("TWITCH_NICK")?;
    let oauth = env::var("TWITCH_OAUTH")?;
    let channel = env::var("TWITCH_CHANNEL")?;
    let database = env::var("DATABASE_URL")?;
    eprintln!("{}", &database);

    let pool = initialize_db_pool(&database).await?;

    loop {
        log::info!("Starting!");

        log::trace!("Loading environment");

        // make a dispatcher (this is how you 'subscribe' to events)
        // this is clonable, so you can send it to other tasks/threasd
        let dispatcher = twitchchat::Dispatcher::new();

        // make a new runner
        // control allows you to stop the runner, and gives you access to an async. encoder (writer)
        let (runner, control) =
            twitchchat::Runner::new(dispatcher.clone(), twitchchat::RateLimit::default());

        let bot = Bot::create(control, pool.clone())?;
        let bot = bot.run(dispatcher, &channel);

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
                        log::warn!("Connection closed.");
                    }
                    Ok(twitchchat::Status::Canceled) => {
                        log::warn!("Shutting down.");
                        return Ok(())
                    }
                    Err(err) => {
                        log::warn!("Error. {:?}", err);
                    }
                }
            }
        }
        log::info!("Restarting, waiting 1 minute.");
        tokio::time::delay_for(std::time::Duration::from_secs(60)).await;
    }
}

async fn initialize_db_pool(db_url: &str) -> anyhow::Result<sqlx::SqlitePool> {
    let pool = sqlx::Pool::new(db_url).await?;
    sqlx::query_file!("sql/db_schema.sql")
        .execute(&pool)
        .await?;
    Ok(pool)
}
