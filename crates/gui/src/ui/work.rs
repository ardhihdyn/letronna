//! The Work tab: what the agent is doing in the turn on screen, step by
//! step, beside the chat. Each call is a row with what it touched; the one
//! picked -- the newest, unless another was clicked -- opens below into
//! what it changed or printed: an edit's lines, a command's output.

use crate::prelude::*;

/// Lines of a step's output or diff shown, the newest kept.
const SHOWN_LINES: usize = 60;

/// One call of the turn, with its result once in.
pub(crate) struct Step {
    pub(crate) name: String,
    pub(crate) args: String,
    pub(crate) result: Option<String>,
    pub(crate) failed: bool,
    pub(crate) running: bool,
}

/// What a step is, for its icon and how its detail is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Kind {
    Edit,
    Command,
    Browse,
    Read,
    Other,
}

pub(crate) fn kind_of(tool: &str) -> Kind {
    match tool {
        "edit_file" | "write_file" | "apply_patch" => Kind::Edit,
        "run_command" => Kind::Command,
        "browse" | "use_page" | "web_fetch" | "web_search" => Kind::Browse,
        "read_file" | "list_dir" | "glob" | "grep" | "search_files" => Kind::Read,
        _ => Kind::Other,
    }
}

fn icon_of(kind: Kind) -> AppIcon {
    match kind {
        Kind::Edit => AppIcon::Pencil,
        Kind::Command => AppIcon::Terminal,
        Kind::Browse => AppIcon::View,
        Kind::Read => AppIcon::Search,
        Kind::Other => AppIcon::Sparkles,
    }
}

/// The last `n` lines of `text`, with how many were left out before them.
fn tail(text: &str, n: usize) -> (usize, Vec<&str>) {
    let lines: Vec<&str> = text.lines().collect();
    let skip = lines.len().saturating_sub(n);
    (skip, lines[skip..].to_vec())
}

impl Chat {
    /// The calls of the newest turn in the session on screen, in order,
    /// the one running now last.
    pub(crate) fn work_steps(&self) -> Vec<Step> {
        let conversation = &self.store.conversations[self.store.active];
        let messages = &conversation.messages;
        let start = messages
            .iter()
            .rposition(|m| m.role == "user")
            .map_or(0, |i| i + 1);
        let mut steps: Vec<(String, Step)> = Vec::new();
        for m in &messages[start..] {
            if m.role == "assistant" {
                for call in &m.tool_calls {
                    steps.push((
                        call.id.clone(),
                        Step {
                            name: call.name.clone(),
                            args: call.arguments.clone(),
                            result: None,
                            failed: false,
                            running: false,
                        },
                    ));
                }
            } else if m.role == "tool"
                && let Some((_, step)) = steps.iter_mut().find(|(id, _)| *id == m.tool_call_id)
            {
                step.result = Some(m.content.clone());
                step.failed = m.is_error || crate::ui::messages::exited_badly(m);
            }
        }
        let here = self.busy && self.streaming_id == Some(conversation.id);
        if here && let Some(turn) = self.turn.as_ref() {
            for call in &turn.live.calls {
                if !steps.iter().any(|(id, _)| *id == call.id) {
                    steps.push((
                        call.id.clone(),
                        Step {
                            name: call.name.clone(),
                            args: call.arguments.clone(),
                            result: None,
                            failed: false,
                            running: false,
                        },
                    ));
                }
            }
            if let Some((_, step)) = steps.iter_mut().rev().find(|(_, s)| s.result.is_none()) {
                step.running = true;
            }
        }
        steps.into_iter().map(|(_, step)| step).collect()
    }

    /// Whether the turn on screen has any step to show.
    pub(crate) fn has_work(&self) -> bool {
        !self.work_steps().is_empty()
    }

    pub(crate) fn work_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let steps = self.work_steps();
        let theme = cx.theme();
        let (muted, border, fg) = (theme.muted_foreground, theme.border, theme.foreground);
        let running = steps.iter().any(|s| s.running);
        let picked = self
            .work_pick
            .filter(|&i| i < steps.len())
            .unwrap_or(steps.len().saturating_sub(1));
        let failed = steps.iter().filter(|s| s.failed).count();
        let head = h_flex()
            .px_4()
            .py_3()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(if running {
                crate::ui::busy::Spinner::new()
                    .xsmall()
                    .color(theme.primary)
                    .into_any_element()
            } else {
                Icon::new(if failed > 0 {
                    IconName::CircleX
                } else {
                    IconName::CircleCheck
                })
                .size_4()
                .text_color(if failed > 0 {
                    theme.danger
                } else {
                    theme.success
                })
                .into_any_element()
            })
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(if running { "Working" } else { "Done" }),
            )
            .child(div().text_xs().text_color(muted).child(match steps.len() {
                1 => "1 step".to_owned(),
                n => format!("{n} steps"),
            }));
        let mut list = v_flex().gap_1().p_2();
        for (i, step) in steps.iter().enumerate() {
            let kind = kind_of(&step.name);
            let open = i == picked;
            let summary = letronna_core::tools::summary(&step.name, &step.args);
            let status: AnyElement = if step.running {
                crate::ui::busy::Spinner::new()
                    .xsmall()
                    .color(theme.primary)
                    .into_any_element()
            } else if step.failed {
                Icon::new(IconName::Close)
                    .size_3p5()
                    .text_color(theme.danger)
                    .into_any_element()
            } else if step.result.is_some() {
                Icon::new(IconName::Check)
                    .size_3p5()
                    .text_color(muted)
                    .into_any_element()
            } else {
                div().into_any_element()
            };
            let row = h_flex()
                .id(("work-step", i))
                .h(px(34.))
                .px_2()
                .gap_2()
                .items_center()
                .rounded(theme.radius)
                .cursor_pointer()
                .when(open, |this| this.bg(theme.secondary))
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    div()
                        .size(px(22.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(7.))
                        .bg(theme.primary.opacity(0.10))
                        .child(Icon::new(icon_of(kind)).size_3p5().text_color(theme.link)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(fg)
                        .flex_none()
                        .child(step.name.replace('_', " ")),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .font_family(MONO_FONT)
                        .text_color(muted)
                        .child(summary),
                )
                .child(status)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.work_pick = Some(i);
                    cx.notify();
                }));
            let row = if step.running {
                row.with_animation(
                    ("work-in", i),
                    crate::ui::motion::enter(std::time::Duration::from_millis(260)),
                    |el, t| el.opacity(t),
                )
                .into_any_element()
            } else {
                row.into_any_element()
            };
            list = list.child(row);
            if open {
                list = list.child(self.work_detail(step, kind, cx));
            }
        }
        let empty = steps.is_empty().then(|| {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .p_8()
                .child(Icon::new(AppIcon::Terminal).size_6().text_color(muted))
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("What the agent edits and runs shows here as it works."),
                )
        });
        v_flex()
            .size_full()
            .child(head)
            .child(
                div()
                    .id("work-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list)
                    .children(empty),
            )
            .into_any_element()
    }

    /// What step `step` changed or printed, opened under its row.
    fn work_detail(&self, step: &Step, kind: Kind, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, added, removed) = (theme.muted_foreground, theme.success, theme.danger);
        let frame = v_flex()
            .mx_2()
            .mb_2()
            .p_3()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .text_xs()
            .font_family(MONO_FONT)
            .gap_0p5();
        let edited = (kind == Kind::Edit)
            .then(|| crate::ui::messages::edited_by(&step.name, &step.args))
            .flatten();
        if let Some(edited) = edited {
            let (skip, lines) = tail(&edited.text, SHOWN_LINES);
            let frame = frame.child(
                h_flex()
                    .gap_2()
                    .pb_1()
                    .child(div().text_color(added).child(format!("+{}", edited.added)))
                    .child(
                        div()
                            .text_color(removed)
                            .child(format!("−{}", edited.removed)),
                    ),
            );
            let frame = frame.when(skip > 0, |this| {
                this.child(
                    div()
                        .text_color(muted)
                        .child(format!("⋯ {skip} lines above")),
                )
            });
            return frame
                .children(lines.into_iter().map(|line| {
                    let (bg, color) = if line.starts_with('+') {
                        (added.opacity(0.10), added)
                    } else if line.starts_with('-') {
                        (removed.opacity(0.10), removed)
                    } else {
                        (gpui::transparent_black(), muted)
                    };
                    div()
                        .px_1()
                        .rounded(px(3.))
                        .bg(bg)
                        .text_color(color)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child(line.to_owned())
                }))
                .into_any_element();
        }
        let Some(result) = step.result.as_deref() else {
            let words = if step.running && self.asking.is_some() {
                "Waiting for you to allow it, in the chat."
            } else if step.running {
                "Running…"
            } else {
                "Waiting to run."
            };
            return frame
                .child(div().text_color(muted).child(words))
                .into_any_element();
        };
        let (skip, lines) = tail(result.trim_end(), SHOWN_LINES);
        let color = if step.failed {
            theme.danger
        } else {
            theme.foreground
        };
        frame
            .when(kind == Kind::Command, |this| {
                this.child(div().pb_1().text_color(theme.link).child(format!(
                    "$ {}",
                    letronna_core::tools::summary(&step.name, &step.args)
                )))
            })
            .when(skip > 0, |this| {
                this.child(
                    div()
                        .text_color(muted)
                        .child(format!("⋯ {skip} lines above")),
                )
            })
            // Wrapped, so a long error is read whole rather than cut at
            // the panel's edge.
            .children(
                lines
                    .into_iter()
                    .map(|line| div().text_color(color).child(line.to_owned())),
            )
            .when(result.trim().is_empty(), |this| {
                this.child(div().text_color(muted).child("No output."))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, kind_of, tail};

    #[test]
    fn a_step_is_told_by_its_tool() {
        assert_eq!(kind_of("edit_file"), Kind::Edit);
        assert_eq!(kind_of("run_command"), Kind::Command);
        assert_eq!(kind_of("browse"), Kind::Browse);
        assert_eq!(kind_of("anything"), Kind::Other);
    }

    #[test]
    fn only_the_newest_lines_are_kept() {
        assert_eq!(tail("a\nb\nc", 2), (1, vec!["b", "c"]));
        assert_eq!(tail("a", 5), (0, vec!["a"]));
    }
}
