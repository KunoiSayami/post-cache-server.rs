use std::time::{Duration, Instant};

use axum::{http::HeaderValue, response::IntoResponse};
use moka::Expiry;

pub enum ErrorEnum {
    System,
    Remote(reqwest::Error),
    NoError,
}

impl From<reqwest::Error> for ErrorEnum {
    fn from(value: reqwest::Error) -> Self {
        Self::Remote(value)
    }
}

pub struct CacheResponse {
    err: ErrorEnum,
    resp: Vec<u8>,
    hit: bool,
}

impl CacheResponse {
    pub fn new(resp: Vec<u8>, hit: bool) -> Self {
        Self {
            err: ErrorEnum::NoError,
            resp,
            hit,
        }
    }

    pub fn from_err(err: ErrorEnum) -> Self {
        Self {
            err,
            resp: vec![],
            hit: false,
        }
    }
}

impl IntoResponse for CacheResponse {
    fn into_response(self) -> axum::response::Response {
        match self.err {
            ErrorEnum::System => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "System error",
                )
                    .into_response()
            }
            ErrorEnum::Remote(error) => {
                return (axum::http::StatusCode::BAD_GATEWAY, error.to_string()).into_response()
            }
            ErrorEnum::NoError => {}
        }
        (
            [(
                "X-Cache-Hit",
                HeaderValue::from_static(self.hit.then(|| "1").unwrap_or("0")),
            )],
            self.resp,
        )
            .into_response()
    }
}

pub struct KeyExpiry(u64);

impl KeyExpiry {
    pub fn new(time: u64) -> Self {
        Self(time)
    }
}

impl Expiry<u64, Vec<u8>> for KeyExpiry {
    /// Returns the duration of the expiration of the value that was just
    /// created.
    fn expire_after_create(
        &self,
        _key: &u64,
        _value: &Vec<u8>,
        _current_time: Instant,
    ) -> Option<Duration> {
        //println!("MyExpiry: expire_after_create called with key {_key} and value {value:?}.");
        Some(Duration::from_secs(self.0))
    }
}
