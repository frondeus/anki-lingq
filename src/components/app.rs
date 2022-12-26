use dioxus::prelude::*;
use reqwest::Client;
use sir::AppStyle;

use crate::{
    components::{lesson::Lesson, popup::LingQPopup},
    lingq, use_cached_future, Config,
};

#[derive(PartialEq, Props)]
pub struct AppProps {
    pub config: Config,
}

pub fn App(cx: Scope<AppProps>) -> Element {
    cx.provide_context(cx.props.config.clone());
    cx.provide_context(Client::new());

    let (fut, refresh_lessons) = use_cached_future(&cx, "./.cache/lessons.json", || {
        let config = cx.consume_context().expect("config");
        let mut client = cx.consume_context().expect("client");
        async move { lingq::get_lessons(&mut client, &config).await }
    });

    let lessons = match fut.value() {
        None => {
            return cx.render(rsx! {
               div { "Loading..." }
            });
        }
        Some(Err(e)) => {
            return cx.render(rsx! {
                div { h1 { "Something went wrong"} "{e}" }
            });
        }
        Some(Ok(o)) => o,
    };

    let lessons = lessons.iter().map(|lesson| {
        rsx! {
            Lesson { lesson: lesson }
        }
    });

    cx.render(rsx! {
        AppStyle {}
        div {
            button { onclick: move |_| { refresh_lessons.refresh() }, "refresh" },
            lessons,
        }
        LingQPopup {}
    })
}
