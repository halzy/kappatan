use kappatan::Bot;

use std::env;
use twitchchat::UserConfig;

struct Config {
    user_config: UserConfig,
    channels: Vec<String>,
    database_url: String,
}

impl Config {
    fn load() -> anyhow::Result<Self> {
        let nick = env::var("TWITCH_NICK")?;
        let oauth = env::var("TWITCH_OAUTH")?;

        let user_config = UserConfig::builder()
            .name(&nick)
            .token(&oauth)
            .enable_all_capabilities()
            .build()?;

        let channels = env::var("TWITCH_CHANNEL")?
            .split(',')
            .map(|s| s.to_string())
            .collect();

        let database_url = env::var("DATABASE_URL")?;

        Ok(Self {
            user_config,
            channels,
            database_url,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load the config
    simple_env_load::load_env_from(&[".env", ".env.production"]);
    alto_logger::init_term_logger()?;

    let config = Config::load()?;

    for channel in &config.channels {
        eprintln!("channels we'll be on: {}", &channel);
    }

    let pool = initialize_db_pool(&config.database_url).await?;

    loop {
        log::info!("Starting!");
        log::trace!("Loading environment");

        match Bot::run_to_completion(pool.clone(), &config.user_config, &config.channels).await {
            Ok(true) => break Ok(()),
            Ok(false) => {
                // we should restart
            }
            Err(err) => {
                // we should restart
                log::error!("ran into an error: {}", err)
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
