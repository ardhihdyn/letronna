use super::sessions::{self, SessionFilter, group_by_project, passes, project_label};
use crate::prelude::*;

/// The sidebar's width, its border included, which the chat column is
/// sized from.
pub(crate) const SIDEBAR_WIDTH: Pixels = px(250.);
/// The rail the sidebar folds to: a column of icons, so New session, search
/// and the pages stay a click away.
pub(crate) const RAIL_WIDTH: Pixels = px(52.);
/// How long the sidebar takes to open out of the rail or fold into it.
const FOLD: std::time::Duration = std::time::Duration::from_millis(240);

/// The sidebar's pages after New session, in the order it lists them, every
/// one a row of its own; Workflows only while there is a run to show.
const PAGES: [Page; 9] = [
    Page::Tasks,
    Page::Capabilities,
    Page::Activity,
    Page::Plugins,
    Page::Messaging,
    Page::Artifacts,
    Page::Jobs,
    Page::Wallet,
    Page::Workflows,
];

/// A row of the sidebar's page list.
#[derive(Clone, Copy, PartialEq)]
enum Nav {
    New,
    Page(Page),
}

impl Nav {
    /// A number of its own, for the row's id and its focus.
    fn slot(self) -> usize {
        match self {
            Nav::New => 0,
            Nav::Page(page) => 1 + page as usize,
        }
    }
}

/// The icon a page's row in the sidebar shows.
fn page_icon(page: Page) -> AppIcon {
    match page {
        Page::Chat => AppIcon::MessageSquare,
        Page::Capabilities => AppIcon::Sparkles,
        Page::Plugins => AppIcon::Puzzle,
        Page::Messaging => AppIcon::MessageSquare,
        Page::Artifacts => AppIcon::File,
        Page::Jobs => AppIcon::Clock,
        Page::Activity => AppIcon::Robot,
        Page::Workflows => AppIcon::Workflow,
        Page::Wallet => AppIcon::Wallet,
        Page::Tasks => AppIcon::Checklist,
        Page::Import => AppIcon::PopIn,
    }
}

/// The sidebar's page rows, top to bottom: New session, then every page.
fn nav_rows(workflows: bool) -> Vec<Nav> {
    std::iter::once(Nav::New)
        .chain(
            PAGES
                .into_iter()
                .filter(|&page| page != Page::Workflows || workflows)
                .map(Nav::Page),
        )
        .collect()
}

impl Chat {
    /// A row of the sidebar's page list, the `at`th of `handles`: Tab
    /// reaches it and Enter opens it as a click does, and Up and Down go on
    /// to the row above or below, from the last into the sessions.
    fn nav_item(
        &self,
        nav: Nav,
        at: usize,
        handles: &std::rc::Rc<[FocusHandle]>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = matches!(nav, Nav::Page(page) if page == self.page);
        let theme = cx.theme();
        let (muted, ring, accent) = (theme.muted_foreground, theme.ring, theme.sidebar_accent);
        let (icon, label) = match nav {
            Nav::New => (Icon::new(AppIcon::SquarePen), "New session"),
            Nav::Page(page) => (Icon::new(page_icon(page)), page.title()),
        };
        let up = at.checked_sub(1).and_then(|up| handles.get(up)).cloned();
        let down = handles.get(at + 1).cloned();
        let row = h_flex()
            .id(("nav", nav.slot()))
            .track_focus(&handles[at])
            .items_center()
            // Always there, so the ring the keys bring moves nothing.
            .border_1()
            .border_color(theme.transparent)
            .cursor_pointer()
            .text_color(theme.sidebar_foreground)
            .focus(|s| s.border_color(ring));
        let row = match nav {
            // The drawer's one big button, as the phone's New chat: a filled
            // pill across the top.
            Nav::New => row
                .h(px(32.))
                .px_2()
                .gap_2p5()
                .rounded(theme.radius)
                .font_weight(FontWeight::MEDIUM)
                .hover(move |s| s.bg(accent.opacity(0.6)))
                .active(move |s| s.bg(accent))
                .child(icon.size_4().text_color(theme.link))
                .child(div().flex_1().min_w_0().truncate().child(label))
                .children(
                    crate::ui::keys::bound_key(&NewSession, cx)
                        .map(|key| Kbd::new(key).text_xs().opacity(0.6)),
                ),
            // A page is an icon in the dock at the drawer's foot, named
            // under the pointer; the sessions have the room.
            Nav::Page(_) => row
                .size(px(28.))
                .justify_center()
                .rounded(theme.radius)
                .when(active, |this| this.bg(accent))
                .when(!active, |this| {
                    this.hover(move |s| s.bg(accent.opacity(0.6)))
                })
                .active(|s| s.bg(accent.opacity(0.8)))
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(label).build(window, cx)
                })
                .child(icon.size_4().text_color(if active {
                    theme.sidebar_foreground
                } else {
                    muted
                })),
        };
        row
            // As on a session row, a press leaves the keys where they were;
            // Tab or Up and Down are what put them here.
            .on_any_mouse_down(|_, window, _| window.prevent_default())
            .on_click(cx.listener(move |this, _, window, cx| match nav {
                Nav::New => this.new_chat(window, cx),
                Nav::Page(page) => this.open_page(page, window, cx),
            }))
            .on_action(
                cx.listener(move |_, _: &sessions::PreviousSession, window, cx| {
                    if let Some(up) = &up {
                        window.focus(up, cx);
                    }
                }),
            )
            .on_action(cx.listener(
                move |this, _: &sessions::NextSession, window, cx| match &down {
                    Some(down) => window.focus(down, cx),
                    None => this.down_to_sessions(window, cx),
                },
            ))
    }

    /// Open the sidebar out of its rail, or fold it back into it.
    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        self.sidebar_moved = Some(cx.background_executor().now());
        cx.notify();
    }

    /// How much of the window the sidebar takes once it has settled.
    pub(crate) fn sidebar_room(&self) -> Pixels {
        if self.sidebar_open {
            SIDEBAR_WIDTH
        } else {
            RAIL_WIDTH
        }
    }

    /// The sidebar as it stands: the whole of it, the rail, or, while it
    /// opens or folds, the whole of it in a box whose width eases between
    /// the two.
    pub(crate) fn sidebar_frame(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = match self.sidebar_moved {
            Some(since) => crate::ui::motion::progress(since, FOLD, window, cx),
            None => 1.,
        };
        if t >= 1. {
            self.sidebar_moved = None;
            self.sidebar_drawn = self.sidebar_room();
            return if self.sidebar_open {
                self.sidebar(window, cx).into_any_element()
            } else {
                self.rail(cx).into_any_element()
            };
        }
        let (from, to) = if self.sidebar_open {
            (RAIL_WIDTH, SIDEBAR_WIDTH)
        } else {
            (SIDEBAR_WIDTH, RAIL_WIDTH)
        };
        self.sidebar_drawn = from + (to - from) * t;
        div()
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .w(self.sidebar_drawn)
            .child(self.sidebar(window, cx))
            .into_any_element()
    }

    /// The sidebar folded: New session, search and every page as an icon
    /// each, named under the pointer.
    fn rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.store.conversations[self.store.active].id;
        let workflows =
            self.page == Page::Workflows || !letronna_core::workflow::runs(session).is_empty();
        let theme = cx.theme();
        let (accent, muted, link) = (theme.sidebar_accent, theme.muted_foreground, theme.link);
        let radius = theme.radius;
        let button = |id: usize, icon: AppIcon, tip: &'static str, on: bool, tint| {
            div()
                .id(("rail", id))
                .size(px(34.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(radius)
                .cursor_pointer()
                .when(on, |this| this.bg(accent))
                .hover(move |s| s.bg(accent.opacity(0.6)))
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(tip).build(window, cx)
                })
                .child(Icon::new(icon).size_4().text_color(tint))
        };
        let rail = v_flex()
            .id("sidebar-rail")
            .w(RAIL_WIDTH)
            .h_full()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .pt_2()
            .pb_2()
            .bg(theme.sidebar)
            .border_r(px(3.))
            .border_color(theme.sidebar_border)
            .child(
                button(0, AppIcon::SquarePen, "New session", false, link)
                    .on_click(cx.listener(|this, _, window, cx| this.new_chat(window, cx))),
            )
            .child(
                button(1, AppIcon::Search, "Search sessions", false, muted).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.toggle_sidebar(cx);
                        this.search
                            .update(cx, |search, cx| search.focus(window, cx));
                    },
                )),
            )
            .child(div().flex_1());
        let mut rail = rail;
        for page in PAGES
            .into_iter()
            .filter(|&page| page != Page::Workflows || workflows)
        {
            let on = page == self.page;
            rail = rail.child(
                button(2 + page as usize, page_icon(page), page.title(), on, muted).on_click(
                    cx.listener(move |this, _, window, cx| this.open_page(page, window, cx)),
                ),
            );
        }
        rail.child(self.settings_button(cx))
    }

    /// Settings, at the end of the pages wherever the sidebar is: a gear
    /// that is always in sight, with its key in the tooltip.
    fn settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let accent = theme.sidebar_accent;
        div()
            .id("sidebar-settings")
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme.radius)
            .cursor_pointer()
            .when(self.settings_open, |this| this.bg(accent))
            .hover(move |s| s.bg(accent.opacity(0.6)))
            .tooltip(|window, cx| {
                gpui_component::tooltip::Tooltip::new("Settings")
                    .action(&OpenSettings, None)
                    .build(window, cx)
            })
            .child(
                Icon::new(IconName::Settings)
                    .size_4()
                    .text_color(theme.muted_foreground),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                if !this.settings_open {
                    this.open_settings(window, cx)
                }
            }))
    }

    /// Go to `page` as its sidebar row does, reading afresh what it lists.
    pub(crate) fn open_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        match page {
            Page::Capabilities => return self.open_capabilities(window, cx),
            Page::Jobs => return self.open_jobs(window, cx),
            // To the list, as the pill goes, even from a transcript.
            Page::Activity => return self.open_activity(window, cx),
            Page::Workflows => return self.open_workflows(window, cx),
            Page::Artifacts => self.read_artifacts(cx),
            Page::Wallet => self.read_wallet(true, cx),
            Page::Plugins => self.reload_plugins(),
            Page::Import => return self.open_import_page(window, cx),
            Page::Chat | Page::Messaging | Page::Tasks => {}
        }
        self.page = page;
        self.focus_composer(window, cx);
        cx.notify();
    }

    /// Down from the last page row: the keys go on to the sessions, at the
    /// first one listed.
    fn down_to_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&first) = self.listed_sessions(cx).first() else {
            return;
        };
        self.picked = Some(self.store.conversations[first].id);
        window.focus(&self.sidebar_focus, cx);
        cx.notify();
    }

    /// Whether the sidebar's keys are on its first session, or it lists
    /// none, so that Up goes on to the page rows above.
    fn at_first_session(&self, cx: &App) -> bool {
        let listed = self.listed_sessions(cx);
        let at = self.picked.and_then(|id| self.store.index_of(id));
        listed.is_empty() || at.is_some_and(|at| listed.first() == Some(&at))
    }

    pub(crate) fn section(&self, label: &'static str, cx: &Context<Self>) -> Div {
        div()
            .mt_3()
            .mb_1()
            .px_2()
            .text_xs()
            .font_family(HEADING_FONT)
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }

    /// A session row; `picked` rings it as the one the sidebar's keys are on.
    ///
    /// Its menu opens from a right-click (a two-finger click or Control and
    /// click on a Mac), a long press, or the "…" that shows while the
    /// pointer is over it. Everything on it names the session by id: the
    /// rows move as sessions are started and deleted.
    ///
    /// Cmd-click (Ctrl-click off a Mac) and Shift-click select rows, as in
    /// a file list. While any are, each row shows a box to tick, a click
    /// ticks it rather than opening the session, and the fill marks the
    /// selected rather than the one on screen.
    pub(crate) fn session_row(
        &self,
        index: usize,
        now: u64,
        picked: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let c = &self.store.conversations[index];
        let id = c.id;
        let selecting = self.selecting();
        let selected = selecting && self.selected.contains(&id);
        let active = index == self.store.active && self.page == Page::Chat && !selecting;
        let menu_open = self.session_menu.as_ref().is_some_and(|open| open.id == id);
        // The session a reply is running in says so, on screen or not.
        let age = if self.busy && self.streaming_id == Some(id) {
            // In the age's colour: the primary is lost on the sidebar.
            crate::ui::busy::Spinner::new()
                .xsmall()
                .color(cx.theme().muted_foreground)
                .into_any_element()
        } else {
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(age_label(c.updated, now))
                .into_any_element()
        };
        let more = Button::new(("session-more", id))
            .plain(cx)
            .xsmall()
            // In the title's colour and a size up: muted and small, as a
            // turn's is, it was all but lost where the age stands.
            .icon(
                Icon::new(IconName::Ellipsis)
                    .size_4()
                    .text_color(cx.theme().sidebar_foreground),
            )
            .tooltip("More")
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                this.open_session_menu(id, event.position(), window, cx);
            }));
        h_flex()
            .id(("session", id))
            .group(SESSION_ROW)
            .relative()
            .h(px(26.))
            .px_2()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .text_sm()
            .when(selected, |this| this.bg(cx.theme().sidebar_accent))
            // The session on screen: its fill sweeps in from the left, with
            // a blue bar standing up at its edge, each time it is picked.
            .when(active, |this| {
                let (shade, bar) = (cx.theme().sidebar_accent, cx.theme().link);
                let highlight = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .rounded(cx.theme().radius)
                    .bg(shade)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(6.))
                            .bottom(px(6.))
                            .w(px(3.))
                            .rounded_full()
                            .bg(bar),
                    );
                if cx.reduce_motion() {
                    this.child(highlight)
                } else {
                    this.child(highlight.with_animation(
                        ("session-pick", id as usize),
                        crate::ui::motion::enter(std::time::Duration::from_millis(280)),
                        |el, t| el.w(relative(0.3 + 0.7 * t)).opacity(t.min(1.)),
                    ))
                }
            })
            .active(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
            .when(selecting, |this| this.child(check_box(selected, cx)))
            // Its marks are in the blue read as text: the fill blue sinks
            // under 3:1 on the midnight frame. Beside the box the dot would
            // only say what the box says, so it goes; the pin stays.
            .when(c.pinned || !selecting, |this| {
                this.child(if c.pinned {
                    Icon::new(AppIcon::Pin)
                        .size_3()
                        .text_color(if active {
                            cx.theme().link
                        } else {
                            cx.theme().muted_foreground
                        })
                        .into_any_element()
                } else {
                    div()
                        .size(px(4.))
                        .rounded_full()
                        .flex_shrink_0()
                        .bg(if active {
                            cx.theme().link
                        } else {
                            cx.theme().muted_foreground.opacity(0.5)
                        })
                        .into_any_element()
                })
            })
            .child(div().flex_1().min_w_0().truncate().child(c.shown_title()))
            // A run beside the chat holds it.
            .when(self.runs.contains_key(&id), |this| {
                this.child(
                    Icon::new(AppIcon::Play)
                        .size_3()
                        .text_color(cx.theme().link),
                )
            })
            // A notice started a turn there, or a run ended, while you were
            // elsewhere.
            .when(self.unseen.contains(&id), |this| {
                this.child(
                    div()
                        .size(px(6.))
                        .rounded_full()
                        .flex_shrink_0()
                        .bg(cx.theme().link),
                )
            })
            // Green, as a running job's chip is: a helper works for it.
            .when(self.helpers_working_in(id), |this| {
                this.child(
                    div()
                        .size(px(6.))
                        .rounded_full()
                        .flex_shrink_0()
                        .bg(cx.theme().green),
                )
            })
            // The age, and in its place while the pointer is over the row
            // or its menu is open, the "…" that opens the menu: found
            // without knowing to right-click.
            .child(
                h_flex()
                    .relative()
                    .h_full()
                    .flex_shrink_0()
                    .min_w(px(20.))
                    .justify_end()
                    // While selecting, the selection bar and the row's menu
                    // act on all of them; a "…" per row would read as one.
                    .child(
                        div()
                            .when(menu_open, |this| this.invisible())
                            .when(!selecting, |this| {
                                this.group_hover(SESSION_ROW, |s| s.invisible())
                            })
                            .child(age),
                    )
                    .child(
                        h_flex()
                            .absolute()
                            .inset_0()
                            .justify_end()
                            .when(!menu_open, |this| this.invisible())
                            .when(!selecting, |this| {
                                this.group_hover(SESSION_ROW, |s| s.visible())
                            })
                            // Its press is its own, neither a long press
                            // nor a click on the row.
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(more),
                    ),
            )
            .when(picked, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().ring),
                )
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    // Nothing above it takes the press, nor the keys.
                    window.prevent_default();
                    cx.stop_propagation();
                    this.session_press = None;
                    this.open_session_menu(id, event.position, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.press_session(id, event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, _| {
                this.move_press(id, event.position);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, _| this.let_go_press(id, true)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, _, _, _| this.let_go_press(id, false)),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                // Let go of after a long press: the menu it opened is the
                // answer, not the session.
                if this.held_press() {
                    return;
                }
                let Some(index) = this.store.index_of(id) else {
                    return;
                };
                let modifiers = event.modifiers();
                if modifiers.shift {
                    return this.select_span(id, window, cx);
                }
                if modifiers.secondary() || this.selecting() {
                    return this.toggle_selected(id, window, cx);
                }
                this.leave_for(index);
                this.store.active = index;
                this.error = None;
                this.page = Page::Chat;
                // The handle is shared across sessions; start at the latest message.
                this.follow();
                this.focus_composer(window, cx);
                this.save();
                cx.notify();
            }))
    }

    /// The open session menu, drawn over everything at the place it opened,
    /// whichever row it came from and wherever that row has moved since.
    fn open_session_menu_layer(&self) -> Option<impl IntoElement> {
        let open = self.session_menu.as_ref()?;
        Some(
            deferred(
                anchored()
                    .position(open.at)
                    .snap_to_window_with_margin(px(8.))
                    .child(open.menu.clone()),
            )
            .with_priority(1),
        )
    }

    /// The sessions the search and the filter let through, by index, the
    /// newest first.
    fn visible_sessions(&self, cx: &App) -> Vec<usize> {
        let query = self.search.read(cx).value().to_lowercase();
        let filter = self.session_filter();
        (0..self.store.conversations.len())
            .rev()
            // A new session is listed once something is said in it.
            .filter(|&i| !untouched(&self.store.conversations[i]))
            .filter(|&i| {
                query.is_empty()
                    || self.store.conversations[i]
                        .title
                        .to_lowercase()
                        .contains(&query)
            })
            .filter(|&i| filter.is_none_or(|f| passes(self.session_state(i), f)))
            .collect()
    }

    /// The sessions the sidebar lists, by index, in the order it draws
    /// them: pinned first, then by project or the newest first.
    pub(crate) fn listed_sessions(&self, cx: &App) -> Vec<usize> {
        drawn_order(
            &self.store.conversations,
            self.visible_sessions(cx),
            self.store.settings.group_by_project,
        )
    }

    /// Move the sidebar's keys to the session above, or with `down` below.
    fn pick_session(&mut self, down: bool, cx: &mut Context<Self>) {
        let listed = self.listed_sessions(cx);
        let at = self.picked.and_then(|id| self.store.index_of(id));
        self.picked = move_in(&listed, at, down).map(|index| self.store.conversations[index].id);
        cx.notify();
    }

    /// Move the sidebar's keys up, or with `down` down, selecting every
    /// session from where they started to where they land, as Shift and an
    /// arrow do in a list.
    fn extend_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        if !self.selecting() {
            self.select_anchor = self.picked;
        }
        self.pick_session(down, cx);
        let Some(to) = self.picked else {
            return;
        };
        let listed: Vec<u64> = (self.listed_sessions(cx).into_iter())
            .map(|index| self.store.conversations[index].id)
            .collect();
        let from = self.select_anchor.filter(|from| listed.contains(from));
        self.select_anchor = Some(from.unwrap_or(to));
        self.selected = sessions::span(&listed, from.unwrap_or(to), to);
    }

    /// Open the session the sidebar's keys are on. The keys stay there, as
    /// on a search hit, for the next.
    fn open_picked_session(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.picked_session(cx) else {
            return;
        };
        self.store.active = index;
        self.error = None;
        self.page = Page::Chat;
        self.follow();
        self.save();
        cx.notify();
    }

    /// The session the sidebar's keys are on, if it is still listed.
    fn picked_session(&self, cx: &App) -> Option<usize> {
        self.picked
            .and_then(|id| self.store.index_of(id))
            .filter(|index| self.listed_sessions(cx).contains(index))
    }

    /// A sidebar key, which acts only while the sidebar itself has the
    /// focus: in its search box, the key is typed instead.
    fn session_key<A: Action>(
        act: fn(&mut Chat, usize, &mut Window, &mut Context<Chat>),
        cx: &Context<Self>,
    ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _: &A, window, cx| {
            if !this.sidebar_focus.is_focused(window) {
                return cx.propagate();
            }
            if let Some(index) = this.picked_session(cx) {
                act(this, index, window, cx);
            }
        })
    }

    /// A sidebar key that moves its keys, which acts only while the sidebar
    /// itself has the focus.
    fn session_move<A: Action>(
        act: fn(&mut Chat, &mut Context<Chat>),
        cx: &Context<Self>,
    ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _: &A, window, cx| {
            if !this.sidebar_focus.is_focused(window) {
                return cx.propagate();
            }
            act(this, cx)
        })
    }

    pub(crate) fn sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = backend::now();
        // Kept while the page is open, so the entry it was opened from stays.
        let session = self.store.conversations[self.store.active].id;
        let workflows =
            self.page == Page::Workflows || !letronna_core::workflow::runs(session).is_empty();
        let rows = nav_rows(workflows);
        let handles: std::rc::Rc<[FocusHandle]> = rows
            .iter()
            .map(|nav| {
                window
                    .use_keyed_state(("nav-focus", nav.slot()), cx, |_, cx| {
                        cx.focus_handle().tab_stop(true)
                    })
                    .read(cx)
                    .clone()
            })
            .collect();
        let mut new = div().px_2().pt_2();
        let mut dock = h_flex().justify_between().px_1p5().py_2();
        for (at, &row) in rows.iter().enumerate() {
            let item = self.nav_item(row, at, &handles, cx);
            match row {
                Nav::New => new = new.child(item),
                Nav::Page(_) => dock = dock.child(item),
            }
        }
        let visible = self.visible_sessions(cx);
        let keyed = self
            .picked
            .and_then(|id| self.store.index_of(id))
            .filter(|_| self.sidebar_focus.is_focused(window));
        let pinned: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|&i| self.store.conversations[i].pinned)
            .collect();
        let not_pinned: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|&i| !self.store.conversations[i].pinned)
            .collect();
        let mut sessions = v_flex().gap_0p5();
        for &i in &pinned {
            sessions = sessions.child(self.session_row(i, now, keyed == Some(i), cx));
        }
        if self.store.settings.group_by_project {
            for (workspace, indices) in group_by_project(&self.store.conversations, &not_pinned) {
                sessions = sessions.child(
                    div()
                        .mt_2()
                        .mb_0p5()
                        .px_2()
                        .text_xs()
                        .font_family(HEADING_FONT)
                        .text_color(cx.theme().muted_foreground)
                        .child(project_label(workspace.as_deref())),
                );
                for i in indices {
                    sessions = sessions.child(self.session_row(i, now, keyed == Some(i), cx));
                }
            }
        } else {
            let mut last_bucket = "";
            for &i in &not_pinned {
                let b = bucket(self.store.conversations[i].updated, now);
                if b != last_bucket {
                    last_bucket = b;
                    sessions = sessions.child(
                        div()
                            .mt_2()
                            .mb_0p5()
                            .px_2()
                            .text_xs()
                            .font_family(HEADING_FONT)
                            .text_color(cx.theme().muted_foreground)
                            .child(b),
                    );
                }
                sessions = sessions.child(self.session_row(i, now, keyed == Some(i), cx));
            }
        }
        if visible.is_empty() {
            // Only a new session nothing was said in: there is none to match.
            let none_yet = self.store.conversations.iter().all(untouched);
            sessions = sessions.child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if none_yet {
                        "No sessions yet"
                    } else {
                        "No sessions match"
                    }),
            );
        }

        v_flex()
            .id("sidebar")
            .track_focus(&self.sidebar_focus)
            .key_context("Sessions")
            // Only F6 gives the sidebar itself the keys; a click leaves
            // them where they were, or with what was clicked.
            .on_any_mouse_down(|_, window, _| window.prevent_default())
            // From the first session, Up goes on to the page rows above.
            .on_action({
                let last = handles.last().cloned();
                cx.listener(move |this, _: &sessions::PreviousSession, window, cx| {
                    if !this.sidebar_focus.is_focused(window) {
                        return cx.propagate();
                    }
                    if this.at_first_session(cx)
                        && let Some(last) = &last
                    {
                        window.focus(last, cx);
                        return cx.notify();
                    }
                    this.pick_session(false, cx)
                })
            })
            .on_action(Self::session_move::<sessions::NextSession>(
                |this, cx| this.pick_session(true, cx),
                cx,
            ))
            .on_action(Self::session_move::<sessions::OpenSession>(
                Chat::open_picked_session,
                cx,
            ))
            .on_action(Self::session_key::<sessions::PinSession>(
                Chat::pin_picked,
                cx,
            ))
            .on_action(Self::session_key::<sessions::RenameSession>(
                Chat::rename_session,
                cx,
            ))
            .on_action(Self::session_key::<sessions::DeleteSession>(
                Chat::delete_picked,
                cx,
            ))
            .on_action(Self::session_key::<sessions::ToggleSelected>(
                Chat::toggle_selected_at,
                cx,
            ))
            .on_action(Self::session_move::<sessions::SelectPrevious>(
                |this, cx| this.extend_selection(false, cx),
                cx,
            ))
            .on_action(Self::session_move::<sessions::SelectNext>(
                |this, cx| this.extend_selection(true, cx),
                cx,
            ))
            .on_action(cx.listener(|this, _: &sessions::SelectAll, window, cx| {
                if !this.sidebar_focus.is_focused(window) {
                    return cx.propagate();
                }
                this.select_all(window, cx);
            }))
            // Ahead of the window's own Escape, which would stop a reply
            // before it got as far as the selection.
            .on_action(
                cx.listener(|this, _: &sessions::ClearSelection, window, cx| {
                    if !this.clear_selection(window, cx) {
                        cx.propagate();
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &sessions::FilterNeedsYou, _, cx| {
                this.toggle_session_filter(SessionFilter::NeedsYou, cx);
            }))
            .on_action(cx.listener(|this, _: &sessions::FilterWorking, _, cx| {
                this.toggle_session_filter(SessionFilter::Working, cx);
            }))
            .on_action(cx.listener(|this, _: &sessions::FilterUnseen, _, cx| {
                this.toggle_session_filter(SessionFilter::Unseen, cx);
            }))
            .on_action(cx.listener(|this, _: &sessions::FilterPinned, _, cx| {
                this.toggle_session_filter(SessionFilter::Pinned, cx);
            }))
            .on_action(cx.listener(|this, _: &sessions::ToggleGrouping, _, cx| {
                this.toggle_grouping(cx);
            }))
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_shrink_0()
            .relative()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r(px(3.))
            .border_color(cx.theme().sidebar_border)
            .child(new)
            .child(
                div().px_2().pt_0p5().pb_1().child(
                    Input::new(&self.search)
                        .xsmall()
                        .h(px(32.))
                        .px_2()
                        .rounded(cx.theme().radius)
                        .appearance(false)
                        .prefix(
                            Icon::new(AppIcon::Search)
                                .size_4()
                                .mr_1()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .suffix(self.session_filter_button(cx)),
                ),
            )
            .when(self.selecting(), |this| {
                this.child(div().pt_1().child(self.selection_bar(cx)))
            })
            .child(
                v_flex()
                    .id("session-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    // In one column, so the heading's top margin is inside
                    // it: gpui measures a scroll's content from its first
                    // child's edge, and the last row was cut by that margin.
                    .child(
                        v_flex()
                            .flex_shrink_0()
                            .children(self.runs_section(cx))
                            .child(sessions)
                            .children(self.message_hits(cx)),
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().sidebar_accent)
                    .child(dock.child(self.settings_button(cx))),
            )
            .children(self.ring(crate::ui::keys::Region::Sidebar, cx))
            .children(self.open_session_menu_layer())
    }
}

/// The group a session row's hover is told by, for its "…" button.
const SESSION_ROW: &str = "session-row";

/// The box a session row shows while sessions are selected, ticked when
/// it is one of them; in the blue the row's marks use.
fn check_box(on: bool, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .size(px(12.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .border_color(if on {
            theme.link
        } else {
            theme.muted_foreground.opacity(0.6)
        })
        .when(on, |this| {
            this.bg(theme.link).child(
                Icon::new(IconName::Check)
                    .size(px(10.))
                    .text_color(theme.background),
            )
        })
}

/// The `visible` sessions in the order the sidebar draws them: pinned
/// first, then by project with `by_project`, else as they come.
fn drawn_order(
    conversations: &[Conversation],
    visible: Vec<usize>,
    by_project: bool,
) -> Vec<usize> {
    let (mut listed, not_pinned): (Vec<usize>, Vec<usize>) =
        visible.into_iter().partition(|&i| conversations[i].pinned);
    if by_project {
        let groups = group_by_project(conversations, &not_pinned);
        listed.extend(groups.into_iter().flat_map(|(_, indices)| indices));
    } else {
        listed.extend(not_pinned);
    }
    listed
}

/// The session after `picked` in `listed`, or with `down` false the one
/// before, no further than the ends; from none, the first.
fn move_in(listed: &[usize], picked: Option<usize>, down: bool) -> Option<usize> {
    let last = listed.len().checked_sub(1)?;
    let at = picked.and_then(|index| listed.iter().position(|&i| i == index));
    let next = match at {
        None => 0,
        Some(at) if down => (at + 1).min(last),
        Some(at) => at.saturating_sub(1),
    };
    Some(listed[next])
}

#[cfg(test)]
mod tests {
    // Not `use super::*`: the prelude re-exports gpui, whose `test` macro
    // would shadow the built-in attribute.
    use super::{Nav, drawn_order, move_in, nav_rows};
    use letronna_core::store::Conversation;
    use std::path::PathBuf;

    /// The rows by the names they show.
    fn names(rows: &[Nav]) -> Vec<&'static str> {
        rows.iter()
            .map(|row| match row {
                Nav::New => "New session",
                Nav::Page(page) => page.title(),
            })
            .collect()
    }

    #[test]
    fn every_page_has_a_row_with_workflows_only_while_runs_show() {
        let mut all = vec![
            "New session",
            "Tasks",
            "Capabilities",
            "Activity",
            "Plugins",
            "Messaging",
            "Artifacts",
            "Scheduled jobs",
            "Wallet",
        ];
        assert_eq!(names(&nav_rows(false)), all);
        all.push("Workflows");
        assert_eq!(names(&nav_rows(true)), all);
    }

    #[test]
    fn every_page_row_has_a_focus_of_its_own() {
        let rows = nav_rows(true);
        let mut slots: Vec<usize> = rows.iter().map(|row| row.slot()).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), rows.len());
    }

    #[test]
    fn the_sidebar_keys_move_in_its_order_and_stop_at_the_ends() {
        // Listed newest first, so index 7 is at the top.
        let listed = [7, 2, 5];
        assert_eq!(move_in(&listed, Some(7), true), Some(2));
        assert_eq!(move_in(&listed, Some(2), false), Some(7));
        assert_eq!(move_in(&listed, Some(7), false), Some(7));
        assert_eq!(move_in(&listed, Some(5), true), Some(5));
        // From a session the search has hidden, or none, to the top.
        assert_eq!(move_in(&listed, Some(9), true), Some(7));
        assert_eq!(move_in(&listed, None, false), Some(7));
        assert_eq!(move_in(&[], None, true), None);
    }

    #[test]
    fn the_sidebar_keys_go_in_the_order_it_draws_pinned_first() {
        let session = |pinned, workspace: Option<&str>| Conversation {
            pinned,
            workspace: workspace.map(PathBuf::from),
            ..Default::default()
        };
        let conversations = [
            session(false, Some("/code/a")),
            session(false, Some("/code/b")),
            session(true, Some("/code/b")),
            session(false, Some("/code/a")),
        ];
        // As the sidebar has them, newest first.
        let visible = vec![3, 2, 1, 0];
        assert_eq!(
            drawn_order(&conversations, visible.clone(), false),
            [2, 3, 1, 0]
        );
        assert_eq!(drawn_order(&conversations, visible, true), [2, 3, 0, 1]);
    }
}
