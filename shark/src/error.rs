use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AylaError {
    #[error("Failed to login: status code {0} -- {1}")]
    LoginError(StatusCode, String),
    #[error("Failed to refresh token: status code {0} -- {1}")]
    RefreshTokenError(StatusCode, String),
    #[error("Failed to logout: status code {0} -- {1}")]
    LogoutError(StatusCode, String),
    #[error("reqwest error")]
    ReqwestError(#[from] reqwest::Error),
}

#[derive(Error, Debug)]
pub enum SharkError {
    #[error("Ayla Client error: {0}")]
    AylaError(#[from] AylaError),
    #[error("Shark API error: status code {0} -- {1}")]
    ApiError(StatusCode, String),
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}
