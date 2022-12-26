use std::str::MatchIndices;

use crate::lingq;
use dioxus::prelude::*;
use itertools::Itertools;
use sir::css;

#[derive(Default)]
pub enum PopupState {
    #[default]
    Closed,
    Opened {
        lingq: lingq::LingQ,
    },
}

pub static POPUP: AtomRef<PopupState> = |_| PopupState::default();

#[derive(PartialEq, Props)]
pub struct LingQPopupProps {}

enum Fragment<'a> {
    Term(&'a str),
    Rest(&'a str),
}

pub fn LingQPopup<'a>(cx: Scope<LingQPopupProps>) -> Element {
    let popup = use_atom_ref(&cx, POPUP);
    let lingq = match &*popup.read() {
        PopupState::Closed => return None,
        PopupState::Opened { lingq } => lingq.clone(),
    };
    let hints = lingq.hints.iter().map(|hint| {
        rsx! {
            h4 {
               "{hint.text}"
            }
        }
    });
    let style = css!(
        "
        position: fixed;
        z-index: 1;
        left: 0;
        top: 0;
        width: 100%;
        height: 100%;
        overflow: auto;
        background-color: rgba(0,0,0,0.4);
        "
    );

    let content_style = css!(
        "
        width: 80%;
        margin: 15% auto;
        background-color: #fefefe;
        padding: 1em;
        border: 1px solid #888;      
        "
    );
    let fragment_style = css!(
        "
            text-align: center;
        "
    );
    let hints_style = css!(
        "
            text-align: center;
        "
    );
    let term = &lingq.term;
    let lower_fragment = lingq.fragment.to_lowercase();

    let fragments = SplitFragment::new(&lingq.fragment, lower_fragment.match_indices(term));

    let fragment = fragments.map(|f| match f {
        Fragment::Term(fragment) => rsx! { b { "{fragment}" } },
        Fragment::Rest(fragment) => rsx! { "{fragment}"},
    });

    cx.render(rsx! {
        div { class: "{style}", onclick: |_| {
            *popup.write() = PopupState::Closed
        },
            div { class: "{content_style}", onclick: |e| {
              e.cancel_bubble();
            },
                div { class: "{fragment_style}",
                    fragment
                }
                div { class: "{hints_style}",
                    hints
                }
            }
        }
    })
}

struct SplitFragment<'a, I>
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
