use std::sync::OnceLock;

use clap::arg;
use config::Config;
use http::route;
use leveldb::{LevelDB, PersistentStorageHelper};
use log::warn;
use tokio::sync::watch;
use types::KeyExpiry;

mod config;
mod http;
mod leveldb;
mod types;

static STORAGE_HELPER: OnceLock<PersistentStorageHelper> = OnceLock::new();

async fn async_main(config_path: &str) -> anyhow::Result<()> {
    let config = Config::try_read(config_path).await?;
    let (exit_signal_sender, exit_receiver) = watch::channel(false);
    let (cache_map, storage_helper, db_handler) = LevelDB::run(config.cache_directory())?;

    STORAGE_HELPER.set(storage_helper.clone()).unwrap();

    let cache = moka::future::CacheBuilder::<_, Vec<u8>, _>::new(65536)
        .async_eviction_listener(|key, _, _| {
            Box::pin(async move {
                let helper = STORAGE_HELPER.get().unwrap();
                helper.delete(*key).await;
            })
        })
        .initial_capacity(cache_map.len())
        .expire_after(KeyExpiry::new(config.expire_time()))
        .build();

    for (key, value) in cache_map {
        cache.insert(key, value).await;
    }

    let web = tokio::spawn(route(config, cache, storage_helper.clone(), exit_receiver));

    tokio::select! {
        _ = async {
            tokio::signal::ctrl_c().await.ok();
            exit_signal_sender.send(true).ok();
            tokio::signal::ctrl_c().await.ok();
            warn!("Force exit");
        } => {}

        ret = web => {
            ret??;
        }
    }

    storage_helper.exit().await;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        ret = db_handler.join() => {
            ret?;
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let matches = clap::command!()
        .args(&[arg!([CONFIG] "Configure file to read").default_value("config.toml")])
        .get_matches();
    env_logger::Builder::from_default_env().init();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main(matches.get_one::<String>("CONFIG").unwrap()))
}
