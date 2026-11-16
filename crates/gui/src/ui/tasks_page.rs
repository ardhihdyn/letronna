//! The Tasks page: the checklists of every session, by project, so what is
//! in hand, waiting and done across a project is one look rather than a
//! session at a time. A row opens its session.

use super::PageColumn;
use crate::prelude::*;
use std::path::PathBuf;

/// One session's checklist, under the project it works in.
pub(crate) struct Listed {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) doing: Vec<String>,
    pub(crate) pending: Vec<String>,
    pub(crate) done: usize,
}

/// Sessions with a checklist, grouped by their project folder (None for
/// sessions without one), each project's sessions in the order given;
/// sessions with every item done come after the rest of their project.
pub(crate) fn by_project(
    sessions: &[(
        u64,
        String,
        Option<PathBuf>,
        Vec<letronna_core::store::Todo>,
    )],
) -> Vec<(Option<PathBuf>, Vec<Listed>)> {
    let mut groups: Vec<(Option<PathBuf>, Vec<Listed>)> = Vec::new();
    for (id, title, workspace, todos) in sessions {
        if todos.is_empty() {
            continue;
        }
        let of = |status: &str| -> Vec<String> {
            (todos.iter())
                .filter(|t| t.status == status)
                .map(|t| t.text.clone())
                .collect()
        };
        let listed = Listed {
            id: *id,
            title: title.clone(),
            doing: of("doing"),
            pending: of("pending"),
            done: todos.iter().filter(|t| t.status == "done").count(),
        };
        match groups.iter_mut().find(|(ws, _)| ws == workspace) {
            Some((_, list)) => list.push(listed),
            None => groups.push((workspace.clone(), vec![listed])),
        }
    }
    for (_, list) in &mut groups {
        list.sort_by_key(|l| l.doing.is_empty() && l.pending.is_empty());
    }
    groups
}

impl Chat {
    pub(crate) fn tasks_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sessions: Vec<_> = (self.store.conversations.iter())
            .map(|c| (c.id, c.title.clone(), c.workspace.clone(), c.todos.clone()))
            .collect();
        let groups = by_project(&sessions);
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let mut column = v_flex().page_column().pb_6().gap_6();
        for (workspace, list) in &groups {
            let name = workspace
                .as_ref()
                .and_then(|w| w.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "No project".to_owned());
            let open = list
                .iter()
                .map(|l| l.doing.len() + l.pending.len())
                .sum::<usize>();
            let mut section = v_flex().gap_2().child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(AppIcon::Folder).size_4().text_color(muted))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(div().text_xs().text_color(muted).child(match open {
                        0 => "all done".to_owned(),
                        1 => "1 open".to_owned(),
                        n => format!("{n} open"),
                    })),
            );
            for l in list {
                let id = l.id;
                let total = l.doing.len() + l.pending.len() + l.done;
                let finished = l.doing.is_empty() && l.pending.is_empty();
                let item = |text: &String, doing: bool| {
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_sm()
                        .child(
                            div()
                                .size(px(8.))
                                .flex_shrink_0()
                                .rounded_full()
                                .when(doing, |d| d.bg(theme.link))
                                .when(!doing, |d| d.border_1().border_color(muted)),
                        )
                        .child(div().min_w_0().truncate().child(text.clone()))
                };
                section = section.child(
                    v_flex()
                        .id(("task-session", id as usize))
                        .gap_2()
                        .px_4()
                        .py_3()
                        .rounded(theme.radius_lg)
                        .border_1()
                        .border_color(border)
                        .bg(theme.secondary)
                        .cursor_pointer()
                        .hover(|s| s.border_color(theme.ring))
                        .when(finished, |s| s.opacity(0.6))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(l.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("{} of {total} done", l.done)),
                                ),
                        )
                        .children(l.doing.iter().map(|t| item(t, true)))
                        .children(l.pending.iter().take(4).map(|t| item(t, false)))
                        .when(l.pending.len() > 4, |s| {
                            s.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("and {} more", l.pending.len() - 4)),
                            )
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.go_to_session(id, window, cx)
                        })),
                );
            }
            column = column.child(section);
        }
        let blurb = if groups.is_empty() {
            "When the agent plans work as a checklist, each session's list shows here by \
             project. Nothing yet."
        } else {
            "Each session's checklist, by project: what is being done now, what waits, and \
             how far along. Open one to carry on."
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                v_flex()
                    .page_column()
                    .pt_6()
                    .pb_4()
                    .child(super::page_header("Tasks", blurb, [], cx)),
            )
            .child(
                div()
                    .id("tasks-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(column),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::by_project;
    use letronna_core::store::Todo;
    use std::path::PathBuf;

    fn todo(text: &str, status: &str) -> Todo {
        Todo {
            text: text.into(),
            status: status.into(),
        }
    }

    #[test]
    fn sessions_group_by_project_and_finished_ones_sink() {
        let a = Some(PathBuf::from("/a"));
        let sessions = vec![
            (1, "done one".into(), a.clone(), vec![todo("x", "done")]),
            (2, "no list".into(), a.clone(), vec![]),
            (3, "other".into(), None, vec![todo("y", "pending")]),
            (
                4,
                "busy".into(),
                a.clone(),
                vec![todo("z", "doing"), todo("w", "done")],
            ),
        ];
        let groups = by_project(&sessions);
        assert_eq!(groups.len(), 2);
        let ids: Vec<u64> = groups[0].1.iter().map(|l| l.id).collect();
        assert_eq!(ids, [4, 1]);
        assert_eq!(groups[0].1[0].doing, ["z"]);
        assert_eq!(groups[1].1[0].pending, ["y"]);
    }
}
