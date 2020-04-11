#[derive(thiserror::Error, Debug)]
pub enum KappaError {
    #[error("Invalid Input: {0}")]
    BadInput(String),

    #[error("Missing template: {0}")]
    TemplateMissing(String),

    #[error("TemplateError: {0}")]
    TemplateError(#[from] template::markings::Error),

    #[error("TwitchChat Error: {0}")]
    TwitchChatError(#[from] twitchchat::Error),
}
