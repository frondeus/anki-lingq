use itertools::{Either, Itertools};
use std::collections::{HashMap, HashSet};

use dioxus::{core::to_owned, prelude::*};
use reqwest::Client;
use sir::css;

use crate::Config;

use super::{Fragment, SplitFragment};

#[derive(PartialEq, Props)]
pub struct SyncPopupProps {
    lingqs: Vec<crate::lingq::LingQ>,
}

pub fn SyncPopup(cx: Scope<SyncPopupProps>) -> Element {
    let state = use_state(&cx, || SyncState::Default);

    // let failed = use_state(&cx, || HashSet::<usize>::default());
    let anki_error = use_state(&cx, || None);
    let anki_notes = use_state(&cx, || HashMap::<usize, AnkiNoteState>::default());

    use_future(&cx, (), |()| {
        let config = cx.consume_context().expect("config");
        let mut client = cx.consume_context().expect("client");
        to_owned![anki_error, anki_notes];
        async move {
            let notes = get_anki_notes(&config, &mut client).await;
            match notes {
                Ok(notes) => {
                    let notes = notes
                        .into_iter()
                        .map(|id| (id, AnkiNoteState::Known))
                        .collect();
                    anki_notes.set(notes);
                }
                Err(e) => {
                    anki_error.set(Some(e));
                }
            }
        }
    });

    let error_style = css!(
        "
            color: red;
        "
    );
    if let Some(e) = anki_error.get() {
        return cx.render(rsx! {
            div { class: "{error_style}", "{e}"}
        });
    };

    let sync_future = move || {
        let config = cx.consume_context().expect("config");
        let mut client = cx.consume_context().expect("client");
        let lingqs = cx.props.lingqs.clone();
        to_owned![state, anki_notes];
        async move {
            if SyncState::Loading != *state {
                state.set(SyncState::Loading);
                match sync(&config, &mut client, &lingqs, &anki_notes).await {
                    Err(e) => {
                        tracing::error!(%e, "Error!");
                        state.set(SyncState::Error(e.to_string()));
                    }
                    Ok((failed_notes, succeded_notes)) => {
                        tracing::info!("Success!");
                        // failed.set(failed_notes);
                        anki_notes.with_mut(|an| {
                            an.extend(
                                failed_notes
                                    .into_iter()
                                    .map(|id| (id, AnkiNoteState::Failed)),
                            );
                            an.extend(
                                succeded_notes
                                    .into_iter()
                                    .map(|id| (id, AnkiNoteState::Synced)),
                            );
                        });
                        state.set(SyncState::Default);
                    }
                }
            }
            ()
        }
    };

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
    let sync_status = match state.get() {
        SyncState::Default => None,
        SyncState::Loading => Some(rsx! {
            span { class: "{loader_style}" }
        }),
        SyncState::Error(err) => Some(rsx! {
            div { class: "{error_style}", "{err}" }
        }),
    };

    let rows = cx.props.lingqs.iter().map(|lingq| {
        rsx! {
            Row { lingq: lingq, notes: anki_notes, key: "{lingq.pk}" }
        }
    });

    let table_style = css!(
        "
        overflow-y: auto;
        height: 80%;
        table {
            width: 100%;
        }
        thead th {
            position: sticky;
            top: 0;
            background: #eee;
        }
    "
    );

    cx.render(rsx! {
        div {
            sync_status,
        }
        div { class: "{table_style}",
            table {
                thead {
                    tr {
                      th { "Term" },
                      th { "Hint" },
                      th { "Status" },
                    },
                },
                tbody {
                    rows
                }
            },
        }
        button { onclick: move |_| {
            cx.spawn( sync_future() );
        },
            "sync"
        },
    })
}

#[derive(Clone, Copy, PartialEq)]
enum AnkiNoteState {
    Known,
    Failed,
    Synced,
}

enum RowStatus {
    New,
    Known,
    Synced,
    Error(String),
}

#[derive(Props)]
struct RowProps<'a> {
    lingq: &'a crate::lingq::LingQ,
    notes: &'a HashMap<usize, AnkiNoteState>,
}
fn Row<'a>(cx: Scope<'a, RowProps<'a>>) -> Element<'a> {
    let lingq = cx.props.lingq;
    let term = &lingq.term;
    let lower_fragment = lingq.fragment.to_lowercase();
    let fragments = SplitFragment::new(&lingq.fragment, lower_fragment.match_indices(term));
    let fragment = fragments.map(|f| match f {
        Fragment::Term(fragment) => rsx! { b { "{fragment}" } },
        Fragment::Rest(fragment) => rsx! { "{fragment}"},
    });
    let hints = lingq.hints.iter().map(|hint| {
        rsx! {
            b { "{hint.text}" }
        }
    });

    let existing_note = cx.props.notes.get(&lingq.pk);
    let status = match existing_note {
        Some(AnkiNoteState::Known) => RowStatus::Known,
        Some(AnkiNoteState::Failed) => RowStatus::Error("Failed to sync".to_owned()),
        Some(AnkiNoteState::Synced) => RowStatus::Synced,
        None => RowStatus::New,
    };

    let error_style = css!(
        "
            color: red;
        "
    );
    let known_style = css!(
        "
            color: green;
        "
    );
    let done_style = css!(
        "
            color: lime;
        "
    );
    let new_style = css!(
        "
            color: blue;
        "
    );

    let status = match status {
        RowStatus::New => rsx! {
        span { class: "{new_style}",
          "New"
        }
          },
        RowStatus::Known => rsx! {
        span { class: "{known_style}",
          "Known"
        }
          },
        RowStatus::Synced => rsx! {
        span { class: "{done_style}",
          "Done"
        }
          },
        RowStatus::Error(e) => rsx! {
        span { class: "{error_style}",
          "{e}"
        }
         },
    };

    cx.render(rsx! {
        tr {
            td { fragment },
            td { hints },
            td { status }
        }
    })
}

async fn get_anki_notes(config: &Config, client: &mut Client) -> anyhow::Result<HashSet<usize>> {
    let notes = crate::anki::get_notes(client, config).await?;
    let existing_notes = notes
        .into_iter()
        .filter_map(|note| note.fields.get("LingQ").map(|field| field.value.clone()))
        .filter_map(|id| id.parse().ok())
        .collect::<HashSet<usize>>();
    Ok(existing_notes)
}

async fn sync(
    config: &Config,
    client: &mut Client,
    lingqs: &[crate::lingq::LingQ],
    existing_notes: &HashMap<usize, AnkiNoteState>,
) -> anyhow::Result<(HashSet<usize>, HashSet<usize>)> {
    let tag = &config.anki_tag;
    let existing_notes: HashSet<usize> = existing_notes
        .iter()
        .filter(|(_, v)| **v != AnkiNoteState::Failed)
        .map(|(k, _)| *k)
        .collect();

    let mut ids = vec![];
    let new_notes: Vec<_> = lingqs
        .into_iter()
        .filter(|lingq| !existing_notes.contains(&lingq.pk))
        .into_iter()
        .map(|lingq| {
            let term = &lingq.term;
            let pk = lingq.pk;
            ids.push(pk);
            let lower_fragment = lingq.fragment.to_lowercase();
            let fragments = SplitFragment::new(&lingq.fragment, lower_fragment.match_indices(term));
            let fragment = fragments
                .map(|f| match f {
                    Fragment::Term(fragment) => format!("<b>{fragment}</b>"),
                    Fragment::Rest(fragment) => fragment.to_owned(),
                })
                .join(" ");
            let hints = lingq.hints.iter().map(|hint| &hint.text).join(", ");
            let front = fragment;
            let back = hints;
            let mut tags = vec![tag.clone()];
            let mut previous = tag.clone();
            for tag in &lingq.tags {
                let tag = tag.replace(" ", "_");
                previous = format!("{previous}::{tag}");
                tags.push(previous.clone());
            }
            crate::anki::NewNote {
                deck_name: "Dansk::ToProcess".to_owned(),
                model_name: "LingQ".to_owned(),
                fields: maplit::hashmap! {
                    "Front".to_owned() => front,
                    "Back".to_owned() => back,
                    "Term".to_owned() => term.to_owned(),
                    "LingQ".to_owned() => pk.to_string()
                },
                tags,
            }
        })
        .collect();

    let result = crate::anki::add_notes(client, config, new_notes).await?;
    // let result: Vec<Option<usize>> = new_notes
    //     .into_iter()
    //     .enumerate()
    //     .map(|(idx, r)| if idx % 2 == 0 { Some(1) } else { None })
    //     .collect();
    dbg!(&result);

    let (failed, succeded): (HashSet<usize>, HashSet<usize>) = result
        .into_iter()
        .zip(ids.into_iter())
        .partition_map(|(res, id)| {
            if res.is_none() {
                Either::Left(id)
            } else {
                Either::Right(id)
            }
        });
    // .filter_map(|(res, id)| res.is_none().then_some(id))
    // .collect();
    // let notes = crate::anki::get_notes(client, config).await?;
    // let existing_notes = notes
    //     .into_iter()
    //     .filter_map(|note| note.fields.get("LingQ").map(|field| field.value.clone()))
    //     .collect::<Vec<String>>();
    Ok((failed, succeded))
}

#[derive(PartialEq)]
enum SyncState {
    Default,
    Loading,
    Error(String),
}
