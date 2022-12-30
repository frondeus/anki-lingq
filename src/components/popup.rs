use std::str::MatchIndices;

use dioxus::prelude::*;
use itertools::Itertools;
use sir::css;

#[derive(Default, Clone)]
pub enum PopupState {
    #[default]
    Closed,
    LingQ {
        lingq: crate::lingq::LingQ,
    },
    Sync {
        lingqs: Vec<crate::lingq::LingQ>,
    },
}

pub static POPUP: AtomRef<PopupState> = |_| PopupState::default();

#[derive(Props)]
struct PopupProps<'a> {
    children: Element<'a>,
}

fn InnerPopup<'a>(cx: Scope<'a, PopupProps<'a>>) -> Element<'a> {
    let popup = use_atom_ref(&cx, POPUP);
    let style = css!(
        "
        position: fixed;
        z-index: 1;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
        overflow: hidden;
        background-color: rgba(0,0,0,0.4);
        "
    );

    let content_style = css!(
        "
        position: relative;
        top: 50%;
        left: 50%;
        width: 80%;
        height: 80%;
        transform: translate(-50%, -50%);
        overflow: auto;
        background-color: #fefefe;
        padding: 1em;
        border: 1px solid #888;      
        "
    );
    cx.render(rsx! {
        div { class: "{style}", onclick: |_| {
            *popup.write() = PopupState::Closed
        },
            div { class: "{content_style}", onclick: |e| {
                e.cancel_bubble();
            },
                &cx.props.children
            }
        }
    })
}

pub fn Popup(cx: Scope) -> Element {
    let popup = use_atom_ref(&cx, POPUP);
    match popup.read().clone() {
        PopupState::Closed => None,
        PopupState::LingQ { lingq } => cx.render(rsx! {
            InnerPopup {
                lingq::LingQPopup { lingq: lingq }
            }
        }),
        PopupState::Sync { lingqs } => cx.render(rsx! {
            InnerPopup {
                sync::SyncPopup { lingqs: lingqs }
            }
        }),
    }
}

mod sync;

mod lingq;

pub enum Fragment<'a> {
    Term(&'a str),
    Rest(&'a str),
}

pub struct SplitFragment<'a, I>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    slice: &'a str,
    iter: I,
    end: usize,
    saved: Option<(usize, &'a str)>,
}

impl<'a, I> SplitFragment<'a, I>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    pub fn new(slice: &'a str, iter: I) -> Self {
        Self {
            slice,
            iter,
            end: 0,
            saved: None,
        }
    }
}

impl<'a, I> Iterator for SplitFragment<'a, I>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    type Item = Fragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((idx, s)) = self.saved.take() {
            self.end = idx + s.len();
            return Some(Fragment::Term(&self.slice[idx..self.end]));
        }

        match self.iter.next() {
            None => {
                let rest = &self.slice[self.end..];
                if rest.len() > 0 {
                    self.end += rest.len();
                    return Some(Fragment::Rest(rest));
                }
                None
            }
            Some((idx, s)) => {
                if self.end < idx {
                    self.saved = Some((idx, s));
                    return Some(Fragment::Rest(&self.slice[self.end..idx]));
                }
                self.end = idx + s.len();
                Some(Fragment::Term(&self.slice[idx..self.end]))
            }
        }
    }
}
