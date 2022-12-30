use dioxus::prelude::*;
use sir::css;

use super::{Fragment, SplitFragment};

#[derive(PartialEq, Props)]
pub struct LingQPopupProps {
    lingq: crate::lingq::LingQ,
}
pub fn LingQPopup(cx: Scope<LingQPopupProps>) -> Element {
    let lingq = &cx.props.lingq;
    let centered_style = css!(
        "
            text-align: center;
        "
    );
    let hints = lingq.hints.iter().map(|hint| {
        rsx! {
            h4 {
               "{hint.text}"
            }
        }
    });
    let term = &lingq.term;
    let lower_fragment = lingq.fragment.to_lowercase();

    let fragments = SplitFragment::new(&lingq.fragment, lower_fragment.match_indices(term));

    let fragment = fragments.map(|f| match f {
        Fragment::Term(fragment) => rsx! { b { "{fragment}" } },
        Fragment::Rest(fragment) => rsx! { "{fragment}"},
    });

    cx.render(rsx! {
        div { class: "{centered_style}",
            fragment
        }
        div { class: "{centered_style}",
            hints
        }
    })
}
