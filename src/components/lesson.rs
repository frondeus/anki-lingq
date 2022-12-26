use dioxus::prelude::*;
use reqwest::Client;
use sir::css;

use crate::{
    components::popup::PopupState, lingq, use_cached_future, use_opt_cached_future, Config,
};

use super::popup::POPUP;

pub struct LingQProps<'a> {
    lingq: &'a lingq::LingQ,
}

#[derive(PartialEq, Props)]
pub struct LessonProps<'a> {
    lesson: &'a lingq::Lesson,
}

pub fn Lesson<'a>(cx: Scope<'a, LessonProps<'a>>) -> Element<'a> {
    let course = &cx.props.lesson.collection_title;
    let title = &cx.props.lesson.title;
    let id = &cx.props.lesson.id;

    let opened_state = use_state(&cx, || false);

    let (lingqs, _) = use_opt_cached_future(
        &cx,
        format!("./.cache/lesson-{id}.json"),
        *opened_state.get(),
        || {
            let config = cx.consume_context().expect("config");
            let mut client = cx.consume_context().expect("client");
            let source = [cx.props.lesson.clone()];
            async move { lingq::get_lingqs(&mut client, &config, &source).await }
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
            lingqs
        }
    })
}
