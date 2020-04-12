#[derive(thiserror::Error, Debug)]
pub enum KappaError {
    #[error("Invalid Input: {0}")]
    BadInput(String),

    #[error("Missing template: {0}")]
    TemplateMissing(String),

    #[error("TwitchChat Error: {0}")]
    TwitchChatError(#[from] twitchchat::Error),

    #[error("Markings: {0}")]
    MarkingsError(#[from] markings::Error),

    #[error("DB Error: {0}")]
    DbError(#[from] sqlx::Error),
}
