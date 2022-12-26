#![allow(unused_imports)]
#![allow(non_snake_case)]

use anyhow::Result;
use clap::Parser;
use components::app::{App, AppProps};
use dioxus::prelude::*;
use from_network::FromNetwork;
use lingq::ExtendedLingQStatus;
use poll_promise::Promise;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    io::BufWriter,
    path::{Path, PathBuf},
};

use crate::lingq::LingQStatus;

mod from_network;

#[derive(Parser, Debug, Clone, PartialEq)]
pub struct Config {
    #[arg(long, env)]
    lingq_api_key: String,

    #[arg(long, default_value_t = ("lingq".to_owned()))]
    anki_tag: String,

    #[arg(long, default_value_t = ("da".to_owned()))]
    lingq_lang: String,

    #[arg(long, default_value_t = 200)]
    lingq_page_size: usize,
}

fn main() {
    tracing_subscriber::fmt::init();
    dioxus::desktop::launch_with_props(
        App,
        AppProps {
            config: Config::parse(),
        },
        |config| config.with_window(|w| w.with_title("LingQ -> Anki")),
    );
}

mod components;

mod anki;
mod lingq;

#[derive(Clone, Copy)]
struct UseRefreshCacheState<'a> {
    state: &'a UseState<u8>,
}

impl<'a> UseRefreshCacheState<'a> {
    pub fn refresh(self) {
        self.state.modify(|c| match c {
            1 => 2,
            _ => 1,
        })
    }
}

fn use_cached_future<'a, C, F, Fut, T>(
    cx: &'a ScopeState,
    cache: C,
    func: F,
) -> (&'a UseFuture<Result<T>>, UseRefreshCacheState<'a>)
where
    C: AsRef<Path> + 'static,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>> + 'static,
    T: DeserializeOwned + Serialize + 'static,
{
    let refresh = use_state(&cx, || 0);

    let should_refresh = refresh.get();
    let fut = use_future(&cx, (should_refresh,), move |(refresh_cache,)| {
        cached(cache, func(), refresh_cache > 0)
    });

    (fut, UseRefreshCacheState { state: refresh })
}

fn use_opt_cached_future<'a, C, F, Fut, T>(
    cx: &'a ScopeState,
    cache: C,
    should_call: bool,
    func: F,
) -> (&'a UseFuture<Option<Result<T>>>, UseRefreshCacheState<'a>)
where
    C: AsRef<Path> + 'static,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>> + 'static,
    T: DeserializeOwned + Serialize + 'static,
{
    let refresh = use_state(&cx, || 0);

    let should_refresh = refresh.get();
    let fut = use_future(
        &cx,
        (should_refresh, &should_call),
        move |(refresh_cache, should_call)| {
            cached_opt(cache, func(), should_call, refresh_cache > 0)
        },
    );

    (fut, UseRefreshCacheState { state: refresh })
}

async fn cached<T: DeserializeOwned + Serialize>(
    cache: impl AsRef<Path>,
    inner: impl Future<Output = Result<T>>,
    refresh: bool,
) -> Result<T> {
    let cache = cache.as_ref();
    let fetch_new = move || async move {
        let t = inner.await?;
        if let Ok(writer) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(cache)
            .map(|f| BufWriter::new(f))
        {
            tracing::info!("Writing to cache");
            if let Err(e) = serde_json::to_writer(writer, &t) {
                tracing::error!(error = ?e, "Could not write to cache");
            }
        }
        Ok::<T, anyhow::Error>(t)
    };

    let result = if refresh {
        fetch_new().await?
    } else {
        match cached_inner(cache).ok() {
            None => fetch_new().await?,
            Some(t) => t,
        }
    };
    Ok(result)
}

async fn cached_opt<T: DeserializeOwned + Serialize>(
    cache: impl AsRef<Path>,
    inner: impl Future<Output = Result<T>>,
    should_call: bool,
    refresh: bool,
) -> Option<Result<T>> {
    if !should_call {
        return None;
    }

    Some(cached(cache, inner, refresh).await)
}

fn cached_inner<T: DeserializeOwned>(cache: impl AsRef<Path>) -> Result<T> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(cache.as_ref())?;
    let reader = std::io::BufReader::new(file);

    let t = serde_json::from_reader(reader)?;
    Ok(t)
}
