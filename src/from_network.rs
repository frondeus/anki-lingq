use std::{
    future::Future,
    io::{BufReader, BufWriter},
    path::PathBuf,
};

use anyhow::Result;
use poll_promise::Promise;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct FromNetwork<T: Send + 'static> {
    #[serde(skip)]
    promise: Option<Promise<Result<T>>>,
    cache: Option<PathBuf>,
    #[serde(skip)]
    first_cache: bool,
}

impl<T> Default for FromNetwork<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self {
            promise: None,
            cache: None,
            first_cache: false,
        }
    }
}

impl<'de, T: Send + 'static + serde::de::DeserializeOwned> Deserialize<'de> for FromNetwork<T> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Inner {
            cache: Option<PathBuf>,
        }
        let inner = Inner::deserialize(de)?;
        Ok(match inner.cache {
            None => Self::default(),
            Some(cache) => Self::init_or_default(cache),
        })
    }
}

// pub trait Loadable: Sized {
//     fn load<R>(reader: BufReader<R>) -> Result<Self>;
// }

impl<T> FromNetwork<T>
where
    T: Send + 'static + serde::de::DeserializeOwned,
{
    pub fn init(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let file = std::fs::OpenOptions::new().read(true).open(path.as_ref())?;
        let reader = std::io::BufReader::new(file);

        let t = serde_json::from_reader(reader)?;

        Ok(Self {
            promise: Some(Promise::from_ready(Ok(t))),
            cache: Some(path.as_ref().into()),
            first_cache: true,
        })
    }

    pub fn init_or_default(path: impl AsRef<std::path::Path>) -> Self {
        tracing::info!(path = %path.as_ref().display(), "Init cache");
        match Self::init(path.as_ref()) {
            Ok(o) => o,
            Err(e) => {
                tracing::error!(error = ?e, "Could not init cache");
                Self {
                    promise: None,
                    cache: Some(path.as_ref().into()),
                    first_cache: false,
                }
            }
        }
    }
}

impl<T> FromNetwork<T>
where
    T: Send + 'static + serde::Serialize,
{
    pub fn invalidate(&mut self) {
        self.promise = None;
        self.first_cache = false;
    }

    pub fn get(&mut self) -> Option<&T> {
        match self.promise.as_ref()?.ready() {
            Some(Ok(t)) => Some(t),
            _ => None,
        }
    }

    // pub fn show<F, Fut>(&mut self, init: F, ui: &mut egui::Ui) -> Option<&mut T>
    // where
    //     F: Fn() -> Fut,
    //     Fut: Future<Output = Result<T>> + Send + 'static,
    // {
    //     let promise = self
    //         .promise
    //         .get_or_insert_with(move || Promise::spawn_async(init()));

    //     match promise.ready_mut() {
    //         None => {
    //             ui.spinner();
    //             None
    //         }
    //         Some(Err(err)) => {
    //             ui.colored_label(egui::Color32::RED, err.to_string());
    //             None
    //         }
    //         Some(Ok(t)) => {
    //             if self.first_cache == false {
    //                 self.first_cache = true;
    //                 if let Some(cache) = self.cache.as_ref() {
    //                     if let Ok(writer) = std::fs::OpenOptions::new()
    //                         .create(true)
    //                         .write(true)
    //                         .truncate(true)
    //                         .open(cache)
    //                         .map(|f| BufWriter::new(f))
    //                     {
    //                         tracing::info!("Writing to cache");
    //                         if let Err(e) = serde_json::to_writer(writer, t) {
    //                             tracing::error!(error = ?e, "Could not write to cache");
    //                         }
    //                     }
    //                 }
    //             }
    //             Some(t)
    //         }
    //     }
    // }
}
