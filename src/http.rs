use std::{sync::Arc, time::Duration};

use axum::{extract::Path, Extension, Json};
use axum_extra::TypedHeader;
use reqwest::{Client, ClientBuilder};
use tokio::sync::watch;
use xxhash_rust::xxh3;

use crate::{config::Config, leveldb::PersistentStorageHelper, types::CacheResponse};

struct Arg {
    client: Client,
    cache: moka::future::Cache<u64, Vec<u8>>,
    helper: PersistentStorageHelper,
    url: String,
}

impl Arg {
    pub fn new(
        cache: moka::future::Cache<u64, Vec<u8>>,
        helper: PersistentStorageHelper,
        url: String,
    ) -> Self {
        Self {
            cache,
            client: ClientBuilder::new()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            helper,
            url,
        }
    }
}

pub async fn route(
    config: Config,
    cache: moka::future::Cache<u64, Vec<u8>>,
    helper: PersistentStorageHelper,
    mut exit_receiver: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let arg = Arc::new(Arg::new(cache, helper, config.upstream().to_string()));

    let router = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async {
                Json(serde_json::json!({"version": env!("CARGO_PKG_VERSION")}))
            }),
        )
        .route("/{*path}", axum::routing::post(handle_post_request))
        .layer(Extension(arg));

    let listener = tokio::net::TcpListener::bind(config.bind()).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            exit_receiver.changed().await.ok();
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        })
        .await?;
    Ok(())
}

async fn request_remote(arg: &Arc<Arg>, path: String, body: String) -> reqwest::Result<String> {
    let ret = arg
        .client
        .post(format!("{}{path}", &arg.url))
        .body(body)
        .send()
        .await?;
    ret.text().await
}

async fn handle_post_request(
    Extension(arg): Extension<Arc<Arg>>,
    cache_header: Option<TypedHeader<axum_extra::headers::CacheControl>>,
    Path(path): Path<String>,
    body: String,
) -> CacheResponse {
    let key = xxh3::xxh3_64(body.as_bytes());

    if !cache_header.is_some_and(|x| x.no_cache()) {
        if let Some(content) = arg.cache.get(&key).await {
            return CacheResponse::new(content, true);
        }
    }
    match request_remote(&arg, path, body).await {
        Ok(ret) => {
            let content = ret.as_bytes().to_vec();
            arg.cache.insert(key, content.clone()).await;
            arg.helper.put(key, content.clone()).await;
            CacheResponse::new(content, false)
        }
        Err(e) => CacheResponse::from_err(e.into()),
    }
}
