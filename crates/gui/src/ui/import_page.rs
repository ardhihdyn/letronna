//! The Import page: sessions from Claude Code, Codex and the other tools
//! Letronna can read, one section a tool, each with its own Scan, to bring
//! one in and carry on here. The same readers as the picker; it only reads.

use super::PageColumn;
use crate::prelude::*;
use letronna_core::import::{self, Candidate, Tool};

/// Sessions read at most per scan.
const SCANNED: usize = 500;
/// Rows a source shows before "Show all".
const FIRST: usize = 8;

/// What the page holds between draws.
#[derive(Default)]
pub(crate) struct ImportState {
    /// Every session found by the last scan, newest first; None before one.
    pub(crate) found: Option<Vec<Candidate>>,
    pub(crate) scanning: bool,
    /// The source the page is narrowed to, or every one.
    pub(crate) only: Option<Tool>,
    /// Sources opened past their first rows.
    pub(crate) all: Vec<Tool>,
    /// The file being read in, so a second press does not import it twice.
    pub(crate) importing: Option<std::path::PathBuf>,
}

/// The tools in `found`, in the order of their first session, with counts.
pub(crate) fn sources(found: &[Candidate]) -> Vec<(Tool, usize)> {
    let mut out: Vec<(Tool, usize)> = Vec::new();
    for c in found {
        match out.iter_mut().find(|(t, _)| *t == c.tool) {
            Some((_, n)) => *n += 1,
            None => out.push((c.tool, 1)),
        }
    }
    // Claude Code and Codex first, the rest as found.
    out.sort_by_key(|(t, _)| match t {
        Tool::ClaudeCode => 0,
        Tool::Codex => 1,
        _ => 2,
    });
    out
}

impl Chat {
    /// Go to the Import page, scanning if it has not yet.
    pub(crate) fn open_import_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = Page::Import;
        if self.imports.found.is_none() {
            self.scan_imports(cx);
        }
        self.focus_composer(window, cx);
        cx.notify();
    }

    pub(crate) fn scan_imports(&mut self, cx: &mut Context<Self>) {
        if self.imports.scanning {
            return;
        }
        self.imports.scanning = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async { import::list(SCANNED) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.imports.found = Some(found);
                this.imports.scanning = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Bring in `candidate` and open it, or open what an earlier import made.
    fn import_one(&mut self, candidate: Candidate, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .store
            .find_by_import(candidate.tool, &candidate.session_id)
        {
            return self.open_imported(index, window, cx);
        }
        if self.imports.importing.is_some() {
            return;
        }
        self.imports.importing = Some(candidate.path.clone());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let imported = cx
                .background_executor()
                .spawn(async move { import::import(&candidate) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.imports.importing = None;
                match imported {
                    Ok(imported) => {
                        let skipped = imported.skipped;
                        let id = this.store.reserve_id();
                        this.store
                            .conversations
                            .push(import::into_conversation(imported, id));
                        let index = this.store.conversations.len() - 1;
                        this.open_imported(index, window, cx);
                        if skipped > 0 {
                            this.toast(
                                format!(
                                    "Import: {skipped} parts could not be read and were left out."
                                ),
                                crate::ui::toast::Kind::Warning,
                                cx,
                            );
                        }
                    }
                    Err(error) => this.toast(
                        format!("Import: {error}"),
                        crate::ui::toast::Kind::Error,
                        cx,
                    ),
                }
            });
        })
        .detach();
    }

    pub(crate) fn import_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let now = backend::now();
        let found = self.imports.found.as_deref().unwrap_or_default();
        let sources = sources(found);
        let scan = Button::new("import-scan")
            .framed(cx)
            .small()
            .icon(Icon::new(AppIcon::Reload))
            .label(if self.imports.scanning {
                "Scanning…"
            } else {
                "Scan again"
            })
            .disabled(self.imports.scanning)
            .on_click(cx.listener(|this, _, _, cx| this.scan_imports(cx)))
            .into_any_element();
        let chip = |id: usize, label: String, on: bool, pick: Option<Tool>| {
            Button::new(("import-src", id))
                .small()
                .when(on, |b| b.primary())
                .when(!on, |b| b.framed(cx))
                .rounded(px(999.))
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.imports.only = pick;
                    cx.notify();
                }))
        };
        let mut chips = h_flex().gap_2().flex_wrap().child(chip(
            0,
            format!("All {}", found.len()),
            self.imports.only.is_none(),
            None,
        ));
        for (n, (tool, count)) in sources.iter().enumerate() {
            chips = chips.child(chip(
                n + 1,
                format!("{} {count}", tool.label()),
                self.imports.only == Some(*tool),
                Some(*tool),
            ));
        }
        let mut column = v_flex().page_column().pb_6().gap_6();
        for (tool, count) in sources
            .iter()
            .filter(|(t, _)| self.imports.only.is_none_or(|o| o == *t))
        {
            let tool = *tool;
            let open = self.imports.all.contains(&tool) || self.imports.only.is_some();
            let mut section = v_flex().gap_2().child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tool.label()),
                    )
                    .child(div().text_xs().text_color(muted).child(match count {
                        1 => "1 session".to_owned(),
                        n => format!("{n} sessions"),
                    })),
            );
            let rows: Vec<&Candidate> = found.iter().filter(|c| c.tool == tool).collect();
            let shown = if open {
                rows.len()
            } else {
                rows.len().min(FIRST)
            };
            for (i, c) in rows.iter().take(shown).enumerate() {
                let done = self.store.find_by_import(c.tool, &c.session_id).is_some();
                let busy = self.imports.importing.as_deref() == Some(c.path.as_path());
                let project = c
                    .workspace
                    .as_ref()
                    .map(|w| {
                        // A path written on Windows reads as one name here.
                        let w = w.to_string_lossy();
                        let w = w.trim_end_matches(['/', '\\']);
                        w.rsplit(['/', '\\']).next().unwrap_or(w).to_owned()
                    })
                    .unwrap_or_else(|| "No project".into());
                let title = match c.title.trim() {
                    "" => "Untitled".to_owned(),
                    t => t.lines().next().unwrap_or(t).to_owned(),
                };
                let candidate = (*c).clone();
                section = section.child(
                    h_flex()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .items_center()
                        .rounded(theme.radius_lg)
                        .border_1()
                        .border_color(border)
                        .bg(theme.secondary)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    div()
                                        .truncate()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(title),
                                )
                                .child(
                                    div().truncate().text_xs().text_color(muted).child(format!(
                                        "{project} · {}",
                                        age_label(c.updated, now)
                                    )),
                                ),
                        )
                        .child(
                            Button::new(("import-one", (tool as usize) * 100_000 + i))
                                .small()
                                .when(done, |b| b.framed(cx).label("Open"))
                                .when(!done, |b| {
                                    b.primary()
                                        .label(if busy { "Importing…" } else { "Import" })
                                })
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.import_one(candidate.clone(), window, cx)
                                })),
                        ),
                );
            }
            if rows.len() > shown {
                section = section.child(
                    Button::new(("import-more", tool as usize))
                        .ghost()
                        .small()
                        .label(format!("Show all {}", rows.len()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.imports.all.push(tool);
                            cx.notify();
                        })),
                );
            }
            column = column.child(section);
        }
        // The two most asked for say so when they have nothing here.
        if self.imports.found.is_some() && self.imports.only.is_none() {
            for (tool, place) in [
                (Tool::ClaudeCode, "~/.claude/projects"),
                (Tool::Codex, "~/.codex/sessions"),
            ] {
                if !sources.iter().any(|(t, _)| *t == tool) {
                    column = column.child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tool.label()))
                            .child(div().text_xs().text_color(muted).child(format!(
                                "No sessions found in {place}. Once {} has run on this computer, Scan again.",
                                tool.label()
                            ))),
                    );
                }
            }
        }
        let blurb = if self.imports.scanning && self.imports.found.is_none() {
            "Looking for sessions from Claude Code, Codex and the other tools on this computer…"
        } else if found.is_empty() {
            "No sessions from Claude Code, Codex, Cursor, Gemini CLI or the others were found \
             on this computer."
        } else {
            "Sessions from other tools on this computer. Import one to carry on here; the \
             original files are only read, never changed."
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                v_flex()
                    .page_column()
                    .pt_6()
                    .pb_4()
                    .gap_4()
                    .child(super::page_header("Import", blurb, [scan], cx))
                    .when(!found.is_empty(), |d| d.child(chips)),
            )
            .child(
                div()
                    .id("import-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(column),
            )
    }
}
