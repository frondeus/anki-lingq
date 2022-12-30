use anyhow::Result;
use dioxus::{core::to_owned, prelude::*};
use reqwest::Client;
use sir::css;

use crate::{
    components::popup::PopupState,
    lingq::{self, ExtendedLingQStatus, LingQ, LingQStatus},
    use_cached_future, use_opt_cached_future, Config,
};

use super::popup::POPUP;

pub struct LingQProps<'a> {
    lingq: &'a lingq::LingQ,
}

#[derive(PartialEq, Props)]
pub struct LessonProps<'a> {
    lesson: &'a lingq::Lesson,
}

async fn get_lingqs(
    client: &mut Client,
    config: &Config,
    source: lingq::Lesson,
) -> Result<Vec<lingq::LingQ>> {
    let source = [source];
    let lingqs: Vec<LingQ> = lingq::get_lingqs(client, config, &source)
        .await?
        .into_iter()
        .filter(|lingq| {
            !(lingq.status == LingQStatus::Known
                && lingq.extended_status == Some(ExtendedLingQStatus::Never))
        })
        .collect();

    Ok(lingqs)
}

pub fn Lesson<'a>(cx: Scope<'a, LessonProps<'a>>) -> Element<'a> {
    let course = &cx.props.lesson.collection_title;
    let title = &cx.props.lesson.title;
    let id = &cx.props.lesson.id;

    let opened_state = use_state(&cx, || false);
    let and_sync = use_state(&cx, || false);

    let (lingqs, _) = use_opt_cached_future(
        &cx,
        format!("./.cache/lesson-{id}.json"),
        *opened_state.get(),
        || {
            let config = cx.consume_context().expect("config");
            let mut client = cx.consume_context().expect("client");
            let lesson = cx.props.lesson.clone();
            async move { get_lingqs(&mut client, &config, lesson).await }
        },
    );

    let popup = use_atom_ref(&cx, POPUP);

    let lingqs = match lingqs.value() {
        None => {
            return cx.render(rsx! {
            span {"..."}});
        }
        Some(None) => None,
        Some(Some(Err(e))) => {
            return cx.render(rsx! {
                div{ h1 { "something went wrong"} "{e}"}
            })
        }
        Some(Some(Ok(o))) => Some(o),
    };
    if *and_sync.get() {
        if let Some(lingqs) = lingqs {
            *popup.write() = PopupState::Sync {
                lingqs: lingqs.clone(),
            };
            *and_sync.make_mut() = false;
        } else {
            *opened_state.make_mut() = true;
        }
    }
    let sync_button = lingqs
        .map(|lingqs| {
            let count = lingqs.len();
            rsx! {
                button { onclick: move |e| {
                    *popup.write() = PopupState::Sync {
                        lingqs: lingqs.clone()
                    };
                    e.cancel_bubble();
                },
                    "sync ({count})"
                },
            }
        })
        .unwrap_or_else(|| {
            rsx! {
                button { onclick: move |e| {
                    *and_sync.make_mut() = true;
                },
                    "sync"
                },
            }
        });
    let rendered_lingqs = lingqs.map(|lingqs| {
        let lingqs = lingqs.iter().map(|lingq| {
            let term = &lingq.term;
            let style = css!(
                "
                display: inline-block;
                border: 1px solid;
                padding: 1em;        
                margin: 0.5em;
            background: #FFF;
            color: #000;
            "
            );
            rsx! {
                div { class: "{style}", onclick: |_| {
                   *popup.write() = PopupState::LingQ {
                        lingq: lingq.clone()
                    };
                },
                      "{term}"
                }
            }
        });

        let style = css!(
            "
           padding: 1em;     
        "
        );

        rsx! {
            div { class: "{style}",
                lingqs
            }
        }
    });

    let style = css!(
        "
        padding: 1em;
        &:nth-child(odd) {
            background: #EEE;
        }
        &:hover {
            background: #99E;
            color: #FFF;
        }
    "
    );

    cx.render(rsx! {
        div { class: "{style}", onclick: |_| {
            tracing::info!("Click!");
            opened_state.modify(|o| !o);
        },
            "{course} - {title}",
            sync_button,
            rendered_lingqs
        }
    })
}
