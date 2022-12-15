use std::result;

pub mod auth;
pub mod tasks;

pub mod files;
pub mod launch;
pub mod metadata;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error(transparent)]
    ZipError(#[from] zip::result::ZipError),
}

pub type Result<T> = result::Result<T, Error>;
