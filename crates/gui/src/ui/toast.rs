//! The window's own toast: one short line in a pill floating over the
//! chat, just above the composer, as the phone says things. It rises in,
//! stays a few seconds -- longer while the pointer is on it, so an error
//! can be read and copied -- and fades away. A newer one takes its place.

use crate::prelude::*;
use std::time::{Duration, Instant};

/// How long a toast stays once nothing holds it.
const STAY: Duration = Duration::from_secs(5);
/// How long it takes to rise in and to fade out.
const MOVE: Duration = Duration::from_millis(220);

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

    /// The toast, placed over the chat above the composer, or nothing.
    pub(crate) fn toast_layer(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let toast = self.toast.as_ref()?;
        let theme = cx.theme();
        let (icon, tint) = match toast.kind {
            Kind::Info => (IconName::Info, theme.link),
            Kind::Warning => (IconName::TriangleAlert, theme.warning),
            Kind::Error => (IconName::CircleX, theme.danger),
        };
        let text = toast.text.clone();
        let copied = toast.copied;
        let leaving = toast.leaving;
        let turn = toast.turn;
        let pill = h_flex()
            .id(("toast", turn as usize))
            .occlude()
            .relative()
            .max_w(px(560.))
            .gap_2p5()
            .items_center()
            .pl_4()
            .pr_1p5()
            .py_1p5()
            .rounded(px(22.))
            .bg(theme.foreground)
            .text_color(theme.background)
            .text_sm()
            .shadow_lg()
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if let Some(toast) = this.toast.as_mut().filter(|t| t.turn == turn) {
                    toast.hovered = *hovered;
                    if !*hovered {
                        // Let go: the full stay again from here.
                        toast.since = Instant::now();
                        this.toast_timer(turn, STAY, cx);
                    }
                }
            }))
            .child(Icon::new(icon).size_4().text_color(tint))
            .child(div().flex_1().min_w_0().line_clamp(2).child(text.clone()))
            .child(
                Button::new("toast-copy")
                    .ghost()
                    .xsmall()
                    .rounded_full()
                    .text_color(theme.background)
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
                    .rounded_full()
                    .text_color(theme.background)
                    .icon(Icon::new(IconName::Close))
                    .tooltip("Dismiss")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toast = None;
                        cx.notify();
                    })),
            );
        let pill = if leaving {
            pill.with_animation(
                ("toast-out", turn as usize),
                crate::ui::motion::exit(MOVE),
                |el, t| el.opacity(1. - t).top(px(8. * t)),
            )
            .into_any_element()
        } else {
            pill.with_animation(
                ("toast-in", turn as usize),
                crate::ui::motion::enter(MOVE),
                |el, t| el.opacity(t).top(px(12. * (1. - t))),
            )
            .into_any_element()
        };
        Some(
            h_flex()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(TOAST_BOTTOM))
                .justify_center()
                .px_6()
                .child(pill),
        )
    }
}

/// How far above the window's foot the toast floats: clear of the
/// composer and the status bar under it.
const TOAST_BOTTOM: f32 = 108.;
