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

#[derive(PartialEq)]
enum SyncState {
    Default,
    Loading,
    Error(String),
}

pub async fn sync(config: &Config, client: &mut Client) -> Result<()> {
    let notes = crate::anki::get_notes(client, config).await?;
    let existing_notes = notes
        .into_iter()
        .filter_map(|note| note.fields.get("LingQ").map(|field| field.value.clone()))
        .collect::<Vec<String>>();
    Ok(())
}

pub fn Lesson<'a>(cx: Scope<'a, LessonProps<'a>>) -> Element<'a> {
    let course = &cx.props.lesson.collection_title;
    let title = &cx.props.lesson.title;
    let id = &cx.props.lesson.id;

    let sync_state = use_state(&cx, || SyncState::Default);
    // let sync_future = use_future(&cx, (&sync_state.clone(),), move |(sync_state,)| {
    //     async move {
    //         match sync_state.get() {
    //             SyncState::Default => (),
    //             SyncState::Loading => sync(&config, &mut client).await,
    //             SyncState::Finished => (),
    //             SyncState::Error(e) => (),
    //         }
    //     }
    // });
    let sync_future = move || {
        let config = cx.consume_context().expect("config");
        let mut client = cx.consume_context().expect("client");
        to_owned![sync_state];
        async move {
            if SyncState::Loading != *sync_state {
                sync_state.set(SyncState::Loading);
                if let Err(e) = sync(&config, &mut client).await {
                    sync_state.set(SyncState::Error(e.to_string()));
                } else {
                    sync_state.set(SyncState::Default);
                }
            }
            ()
        }
    };

    let opened_state = use_state(&cx, || false);

    let (lingqs, _) = use_opt_cached_future(
        &cx,
        format!("./.cache/lesson-{id}.json"),
        *opened_state.get(),
        || {
            let config = cx.consume_context().expect("config");
            let mut client = cx.consume_context().expect("client");
            let source = [cx.props.lesson.clone()];
            async move {
                let lingqs: Vec<LingQ> = lingq::get_lingqs(&mut client, &config, &source)
                    .await?
                    .into_iter()
                    .filter(|lingq| {
                        !(lingq.status == LingQStatus::Known
                            && lingq.extended_status == Some(ExtendedLingQStatus::Never))
                    })
                    .collect();

                Ok(lingqs)
            }
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
    let lingqs_count = lingqs.as_ref().map(|lingqs| lingqs.len()).map(|count| {
        rsx! {
            span { "({count})" }
        }
    });
    let loader_style = css!(
        "
        border: 16px solid #f3f3f3;
        border-top: 16px solid #3498db;
        border-radius: 50%;
        width: 120px;
        height: 120px
        animation: spin 2s linear infinite;  
        "
    );
    let error_style = css!(
        "
            color: red;
        "
    );
    let sync_status = match sync_state.get() {
        SyncState::Default => None,
        SyncState::Loading => Some(rsx! {
            span { class: "{loader_style}" }
        }),
        SyncState::Error(err) => Some(rsx! {
            div { class: "{error_style}", "{err}" }
        }),
    };
    let lingqs = lingqs.map(|lingqs| {
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
                   *popup.write() = PopupState::Opened {
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
            lingqs_count,
            button { onclick: move |_| {
                cx.spawn( sync_future() );
            },
                "sync"
            },
            sync_status,
            lingqs
        }
    })
}
