//! The window's own toast: a heads-up card that drops in from under the
//! title bar over the chat, coloured by what it is. It drops in,
//! stays a few seconds -- longer while the pointer is on it, so an error
//! can be read and copied -- and fades away. A newer one takes its place.

use crate::prelude::*;
use std::time::{Duration, Instant};

/// How long a toast stays once nothing holds it.
const STAY: Duration = Duration::from_secs(5);
/// How long it takes to rise in and to fade out.
const MOVE: Duration = Duration::from_millis(320);

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Info,
    Warning,
    Error,
}

pub(crate) struct Toast {
    text: SharedString,
    kind: Kind,
    /// Bumped with each new toast, so a timer left by an older one does
    /// not take this one away.
    turn: u64,
    /// When it last came in, or was last let go by the pointer.
    since: Instant,
    hovered: bool,
    leaving: bool,
    copied: bool,
    /// Bumped each time the pointer lets go, so the line counting down
    /// starts over from full.
    round: u64,
}

impl Chat {
    /// Say `text` in a toast, in place of any toast already up.
    pub(crate) fn toast(
        &mut self,
        text: impl Into<SharedString>,
        kind: Kind,
        cx: &mut Context<Self>,
    ) {
        let turn = self.toast.as_ref().map_or(0, |t| t.turn + 1);
        self.toast = Some(Toast {
            text: text.into(),
            kind,
            turn,
            since: Instant::now(),
            hovered: false,
            leaving: false,
            copied: false,
            round: 0,
        });
        self.toast_timer(turn, STAY, cx);
        cx.notify();
    }

    /// After `wait`, start this toast fading unless the pointer holds it or
    /// another took its place; then take it away once it has faded.
    fn toast_timer(&self, turn: u64, wait: Duration, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let gone = this.update(cx, |this, cx| {
                let Some(toast) = this.toast.as_mut().filter(|t| t.turn == turn) else {
                    return false;
                };
                if toast.hovered {
                    return false;
                }
                let left = toast.since.elapsed();
                if left < STAY {
                    this.toast_timer(turn, STAY - left, cx);
                    return false;
                }
                toast.leaving = true;
                cx.notify();
                true
            });
            if !matches!(gone, Ok(true)) {
                return;
            }
            cx.background_executor().timer(MOVE).await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .toast
                    .as_ref()
                    .is_some_and(|t| t.turn == turn && t.leaving)
                {
                    this.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The toast, dropped from under the title bar over the chat, or
    /// nothing. Its colour says what kind it is before a word is read: a
    /// filled badge, a tinted edge, and a headline over the message.
    pub(crate) fn toast_layer(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        let theme = cx.theme();
        let (icon, tint) = match toast.kind {
            Kind::Info => (IconName::Info, theme.link),
            Kind::Warning => (IconName::TriangleAlert, theme.warning),
            Kind::Error => (IconName::CircleX, theme.danger),
        };
        let (title, body) = headed(&toast.text, toast.kind);
        let text = toast.text.clone();
        let copied = toast.copied;
        let leaving = toast.leaving;
        let hovered = toast.hovered;
        let (turn, round) = (toast.turn, toast.round);
        let muted = theme.muted_foreground;
        // The time left, as a line along the foot that runs out; held full
        // while the pointer is on the card.
        let countdown = div()
            .h(px(2.))
            .w_full()
            .rounded_full()
            .bg(tint.opacity(0.6));
        let countdown = if hovered || leaving {
            countdown.into_any_element()
        } else {
            countdown
                .with_animation(
                    ("toast-left", (turn * 1000 + round) as usize),
                    Animation::new(STAY),
                    |el, t| el.w(relative(1. - t)),
                )
                .into_any_element()
        };
        let card = v_flex()
            .id(("toast", turn as usize))
            .occlude()
            .relative()
            .overflow_hidden()
            .w(px(400.))
            .max_w_full()
            .gap_1()
            .pl_4()
            .pr_2()
            .pt_2p5()
            .pb_3p5()
            .rounded(px(14.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_xl()
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if let Some(toast) = this.toast.as_mut().filter(|t| t.turn == turn) {
                    toast.hovered = *hovered;
                    if !*hovered {
                        // Let go: the full stay again from here.
                        toast.since = Instant::now();
                        toast.round += 1;
                        this.toast_timer(turn, STAY, cx);
                    }
                    cx.notify();
                }
            }))
            // The kind's colour, as a strip down the left edge.
            .child(
                div()
                    .absolute()
                    .left(px(5.))
                    .top(px(10.))
                    .bottom(px(10.))
                    .w(px(3.))
                    .rounded_full()
                    .bg(tint),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(icon).size_4().text_color(tint))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().text_xs().text_color(muted).child("now"))
                    .child(
                        Button::new("toast-copy")
                            .ghost()
                            .xsmall()
                            .text_color(muted)
                            .icon(Icon::new(if copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            }))
                            .tooltip("Copy")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
                                if let Some(toast) = this.toast.as_mut() {
                                    toast.copied = true;
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toast-close")
                            .ghost()
                            .xsmall()
                            .text_color(muted)
                            .icon(Icon::new(IconName::Close))
                            .tooltip("Dismiss")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toast = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .pl(px(24.))
                    .pr_2()
                    .text_sm()
                    .text_color(muted)
                    .line_clamp(3)
                    .child(body),
            )
            // Inside the corners, so the line never runs past them.
            .child(
                div()
                    .absolute()
                    .left(px(16.))
                    .right(px(16.))
                    .bottom(px(5.))
                    .child(countdown),
            );
        let card = if leaving {
            card.with_animation(
                ("toast-out", turn as usize),
                crate::ui::motion::exit(MOVE),
                |el, t| el.opacity(1. - t).top(px(-16. * t)),
            )
            .into_any_element()
        } else {
            card.with_animation(
                ("toast-in", turn as usize),
                crate::ui::motion::enter(MOVE),
                |el, t| {
                    let t = crate::ui::motion::overshoot(t);
                    el.opacity(t.min(1.)).top(px(-24. * (1. - t)))
                },
            )
            .into_any_element()
        };
        Some(
            h_flex()
                .absolute()
                .left_0()
                .right_0()
                .top(px(TOAST_TOP))
                .justify_center()
                .px_6()
                .child(card),
        )
    }
}

/// How far under the top of the chat the toast drops to.
const TOAST_TOP: f32 = 12.;

/// A toast's title and its body: what comes before the first ": " names
/// where it came from ("Custom: Cannot load…"), and without one the kind
/// says it.
fn headed(text: &str, kind: Kind) -> (SharedString, SharedString) {
    match text.split_once(": ") {
        Some((from, rest)) if !from.is_empty() && from.len() <= 40 && !rest.is_empty() => {
            (from.to_owned().into(), rest.to_owned().into())
        }
        _ => (
            match kind {
                Kind::Info => "Letronna",
                Kind::Warning => "Worth a look",
                Kind::Error => "That did not work",
            }
            .into(),
            text.to_owned().into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, headed};

    #[test]
    fn a_source_before_the_colon_heads_the_toast_and_anything_else_is_its_body() {
        let (title, body) = headed("Custom: Cannot load the model list.", Kind::Error);
        assert_eq!(
            (title.as_ref(), body.as_ref()),
            ("Custom", "Cannot load the model list.")
        );
        let (title, body) = headed("Saved.", Kind::Info);
        assert_eq!((title.as_ref(), body.as_ref()), ("Letronna", "Saved."));
    }
}
