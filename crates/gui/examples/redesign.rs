//! A preview of the 1.0 window, drawn the way the phone app draws Letronna:
//! a quiet bar, a drawer that slides in, the greeting set in Bodoni, your
//! messages in bubbles, the work folded into one card, and a round composer.
//! Nothing here talks to a model; "Show a run" stages one.
//!
//!   cargo run -p letronna-gui --example redesign --release

use gpui::prelude::FluentBuilder;
use gpui::*;
use std::borrow::Cow;
use std::time::Duration;

const SANS: &str = "Barlow";
const SERIF: &str = "Bodoni Moda";
const MONO: &str = "DM Mono";

#[derive(Clone, Copy)]
struct Palette {
    page: u32,
    text: u32,
    muted: u32,
    raised: u32,
    sunk: u32,
    hairline: u32,
    frame: u32,
    frame_on: u32,
    link: u32,
    pill: u32,
    on_pill: u32,
    add: u32,
}

const BLUE: u32 = 0x0040ff;

const NIGHT: Palette = Palette {
    page: 0x11141f,
    text: 0xe3e6f0,
    muted: 0x8c93ad,
    raised: 0x181c2b,
    sunk: 0x0c0f18,
    hairline: 0x272c42,
    frame: 0x080b1e,
    frame_on: 0x1a2551,
    link: 0x6b8cff,
    pill: 0xe3e6f0,
    on_pill: 0x080b1e,
    add: 0x7cf2a8,
};

const PAPER: Palette = Palette {
    page: 0xf2f2f0,
    text: 0x00126b,
    muted: 0x33499b,
    raised: 0xfafaf8,
    sunk: 0xe9e9e5,
    hairline: 0xc6cad8,
    frame: 0xdfe4f1,
    frame_on: 0xcbd6f2,
    link: 0x0040ff,
    pill: 0x00126b,
    on_pill: 0xf2f2f0,
    add: 0x16803c,
};

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// A gentle overshoot, for things that arrive with a little life.
fn spring(t: f32) -> f32 {
    let c = 1.70158;
    1. + (c + 1.) * (t - 1.).powi(3) + c * (t - 1.).powi(2)
}

fn rise<E: Styled>(el: E, d: f32) -> E {
    el.opacity(d).top(px(12. * (1. - d)))
}

#[derive(PartialEq, Clone, Copy)]
enum Stage {
    Empty,
    Asked,
    Thinking,
    Working(usize),
    Answering(usize),
    Done,
}

const STEPS: [(&str, &str); 4] = [
    ("✎", "Edited repos.rs"),
    ("›_", "Ran cargo test"),
    ("◎", "Opened localhost:3000"),
    ("◎", "Clicked on the page"),
];

const REPLY: &str = "The check failed because the repo list was read before the account loaded. \
It now waits for the account, and all 214 tests pass.";

struct Preview {
    dark: bool,
    drawer: bool,
    /// Bumped on each toggle so the drawer's animation starts again.
    drawer_turn: usize,
    stage: Stage,
    open_steps: bool,
    run: usize,
    selected: usize,
}

impl Preview {
    fn p(&self) -> Palette {
        if self.dark { NIGHT } else { PAPER }
    }

    fn start_run(&mut self, cx: &mut Context<Self>) {
        self.run += 1;
        let run = self.run;
        self.stage = Stage::Asked;
        self.open_steps = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let plan: Vec<(u64, Stage)> = vec![
                (700, Stage::Thinking),
                (1400, Stage::Working(1)),
                (650, Stage::Working(2)),
                (650, Stage::Working(3)),
                (650, Stage::Working(4)),
            ];
            for (ms, stage) in plan {
                cx.background_executor()
                    .timer(Duration::from_millis(ms))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.run == run {
                            this.stage = stage;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    return;
                }
            }
            let words = REPLY.split(' ').count();
            for n in 1..=words {
                cx.background_executor()
                    .timer(Duration::from_millis(45))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.run == run {
                        this.stage = if n == words {
                            Stage::Done
                        } else {
                            Stage::Answering(n)
                        };
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn round_button(&self, id: &'static str, glyph: &'static str) -> Stateful<Div> {
        let p = self.p();
        div()
            .id(id)
            .size(px(36.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(rgb(p.hairline))
            .bg(rgb(p.raised))
            .text_color(rgb(p.text))
            .text_size(px(16.))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(p.sunk)))
            .active(|s| s.opacity(0.7))
            .child(glyph)
    }

    fn drawer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.p();
        let chats = [
            "Fix the failing tests",
            "Plan the 1.0 window",
            "Why is the build slow",
            "Draft the release notes",
            "Wallet top-up flow",
        ];
        let open = self.drawer;
        div()
            .h_full()
            .overflow_hidden()
            .bg(rgb(p.frame))
            .child(
                div()
                    .w(px(260.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_4()
                    .pb_4()
                    .pt(px(44.))
                    .child(
                        div()
                            .font_family(SERIF)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(22.))
                            .text_color(rgb(p.text))
                            .pb_4()
                            .child("Letronna"),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.))
                            .text_color(rgb(p.muted))
                            .pb_1()
                            .child("RECENT"),
                    )
                    .children(chats.iter().enumerate().map(|(i, name)| {
                        let on = i == self.selected;
                        div()
                            .id(("chat", i))
                            .px_3()
                            .py_2()
                            .rounded(px(12.))
                            .text_size(px(14.))
                            .text_color(rgb(p.text))
                            .when(on, |d| {
                                d.bg(rgb(p.frame_on)).font_weight(FontWeight::MEDIUM)
                            })
                            .when(!on, |d| d.opacity(0.8))
                            .hover(move |s| s.bg(rgb(p.frame_on)).opacity(1.))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected = i;
                                cx.notify();
                            }))
                            .child(*name)
                            .with_animation(
                                ("chat-in", i * 100 + self.drawer_turn),
                                Animation::new(Duration::from_millis(260 + 50 * i as u64))
                                    .with_easing(ease_out),
                                |el, d| el.opacity(d).left(px(-16. * (1. - d))),
                            )
                    })),
            )
            .with_animation(
                ("drawer", self.drawer_turn),
                Animation::new(Duration::from_millis(280)).with_easing(ease_out),
                move |el, d| el.w(px(260. * if open { d } else { 1. - d })),
            )
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.p();
        let title = if self.stage == Stage::Empty {
            "Letronna"
        } else {
            "Fix the failing tests"
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .child(
                self.round_button("menu", "☰")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.drawer = !this.drawer;
                        this.drawer_turn += 1;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(p.text))
                            .child(title),
                    )
                    .child(
                        div()
                            .id("model")
                            .text_size(px(12.))
                            .text_color(rgb(p.muted))
                            .cursor_pointer()
                            .child("Sonnet 5.5 · Medium ⌄"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        self.round_button("theme", if self.dark { "☀" } else { "☾" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dark = !this.dark;
                                cx.notify();
                            })),
                    )
                    .child(self.round_button("new", "✎").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.run += 1;
                            this.stage = Stage::Empty;
                            cx.notify();
                        },
                    ))),
            )
    }

    fn empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.p();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_size(px(40.))
                    .text_color(rgb(BLUE))
                    .child("✦")
                    .with_animation(
                        ("star", self.run),
                        Animation::new(Duration::from_millis(900)).with_easing(spring),
                        |el, d| el.opacity(d.min(1.)).text_size(px(20. + 20. * d)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .font_family(SERIF)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(34.))
                    .text_color(rgb(p.text))
                    .child("Good afternoon")
                    .with_animation(
                        ("hello", self.run),
                        Animation::new(Duration::from_millis(700)).with_easing(ease_out),
                        rise,
                    ),
            )
            .child(
                div().relative().flex().gap_2().pt_4().children(
                    ["Show a run", "Plan my week", "Review a PR"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, label)| {
                            div()
                                .id(("suggest", i))
                                .relative()
                                .px_4()
                                .py_2()
                                .rounded_full()
                                .border_1()
                                .border_color(rgb(p.hairline))
                                .bg(rgb(p.raised))
                                .text_size(px(13.))
                                .text_color(rgb(p.text))
                                .cursor_pointer()
                                .hover(move |s| s.border_color(rgb(p.link)))
                                .on_click(cx.listener(|this, _, _, cx| this.start_run(cx)))
                                .child(label)
                                .with_animation(
                                    ("suggest-in", i * 1000 + self.run),
                                    Animation::new(Duration::from_millis(600 + 120 * i as u64))
                                        .with_easing(ease_out),
                                    rise,
                                )
                        }),
                ),
            )
    }

    fn thinking_mark(&self, spinning: bool) -> AnyElement {
        let p = self.p();
        let mark = div().text_size(px(16.)).text_color(rgb(p.link)).child("✦");
        if spinning {
            mark.with_animation(
                "spin",
                Animation::new(Duration::from_millis(1100)).repeat(),
                |el, d| el.opacity(0.35 + 0.65 * (d * std::f32::consts::TAU).sin().abs()),
            )
            .into_any_element()
        } else {
            mark.into_any_element()
        }
    }

    fn chat(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.p();
        let stage = self.stage;
        let shown_steps = match stage {
            Stage::Working(n) => n,
            Stage::Answering(_) | Stage::Done => STEPS.len(),
            _ => 0,
        };
        let mut col = div()
            .id("chat")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .px(px(48.))
            .py_4()
            .max_w(px(820.))
            .w_full()
            .mx_auto();

        col = col.child(
            div()
                .relative()
                .flex()
                .justify_end()
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .rounded(px(20.))
                        .bg(rgb(p.raised))
                        .border_1()
                        .border_color(rgb(p.hairline))
                        .text_size(px(15.))
                        .text_color(rgb(p.text))
                        .child("Fix the failing tests"),
                )
                .with_animation(
                    ("ask", self.run),
                    Animation::new(Duration::from_millis(380)).with_easing(spring),
                    |el, d| el.opacity(d.min(1.)).top(px(24. * (1. - d))),
                ),
        );

        if stage == Stage::Asked || stage == Stage::Thinking {
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.thinking_mark(true))
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(p.muted))
                            .child("Thinking…"),
                    ),
            );
        }

        if shown_steps > 0 {
            col = col.child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(14.))
                    .text_color(rgb(p.muted))
                    .child("Thought for 5 s ›")
                    .with_animation(
                        ("thought", self.run),
                        Animation::new(Duration::from_millis(300)),
                        rise,
                    ),
            );
            let done = matches!(stage, Stage::Answering(_) | Stage::Done);
            let header = div()
                .id("steps-head")
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .py_3()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_steps = !this.open_steps;
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(!done, |d| d.child(self.thinking_mark(true)))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(p.text))
                                .child(if done {
                                    "Worked in Letronna Cloud"
                                } else {
                                    "Working in Letronna Cloud"
                                }),
                        ),
                )
                .child(
                    div()
                        .font_family(MONO)
                        .text_size(px(11.))
                        .text_color(rgb(p.muted))
                        .child(format!(
                            "{} step{} {}",
                            shown_steps,
                            if shown_steps == 1 { "" } else { "s" },
                            if self.open_steps { "⌃" } else { "⌄" }
                        )),
                );
            let mut card = div()
                .relative()
                .rounded(px(22.))
                .border_1()
                .border_color(rgb(p.hairline))
                .bg(rgb(p.raised))
                .overflow_hidden()
                .child(header);
            if self.open_steps {
                for (i, (icon, label)) in STEPS.iter().take(shown_steps).enumerate() {
                    let mut row = div()
                        .relative()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_4()
                        .py_2()
                        .border_t_1()
                        .border_color(rgb(p.hairline))
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .font_family(MONO)
                                .text_size(px(12.))
                                .text_color(rgb(p.text))
                                .child(div().w(px(18.)).text_color(rgb(p.muted)).child(*icon))
                                .child(*label),
                        );
                    if i == 0 {
                        row = row.child(
                            div()
                                .ml(px(30.))
                                .p_2()
                                .rounded(px(10.))
                                .bg(rgb(p.sunk))
                                .font_family(MONO)
                                .text_size(px(11.))
                                .text_color(rgb(p.muted))
                                .child("- let repos = list(&account);")
                                .child(
                                    div()
                                        .text_color(rgb(p.add))
                                        .child("+ let account = account.ready().await?;"),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(p.add))
                                        .child("+ let repos = list(&account);"),
                                ),
                        );
                    }
                    card = card.child(row.with_animation(
                        ("step", i * 1000 + self.run),
                        Animation::new(Duration::from_millis(360)).with_easing(ease_out),
                        |el, d| el.opacity(d).left(px(-10. * (1. - d))),
                    ));
                }
            }
            col = col.child(card.with_animation(
                ("card", self.run),
                Animation::new(Duration::from_millis(420)).with_easing(spring),
                |el, d| el.opacity(d.min(1.)).top(px(16. * (1. - d))),
            ));
        }

        let answered = match stage {
            Stage::Answering(n) => Some(n),
            Stage::Done => Some(usize::MAX),
            _ => None,
        };
        if let Some(n) = answered {
            let words: Vec<&str> = REPLY.split(' ').collect();
            let shown = n.min(words.len());
            col = col.child(
                div()
                    .flex()
                    .flex_wrap()
                    .text_size(px(15.))
                    .line_height(px(24.))
                    .text_color(rgb(p.text))
                    .children(words.iter().take(shown).enumerate().map(|(i, w)| {
                        div().mr(px(4.)).child(*w).with_animation(
                            ("w", i * 1000 + self.run),
                            Animation::new(Duration::from_millis(260)).with_easing(ease_out),
                            |el, d| el.opacity(0.2 + 0.8 * d),
                        )
                    })),
            );
        }
        if stage == Stage::Done {
            col = col.child(
                div()
                    .flex()
                    .gap_3()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(rgb(p.muted))
                    .child("COPY")
                    .child("RETRY")
                    .child("12.4 S · $0.03")
                    .with_animation(
                        ("meta", self.run),
                        Animation::new(Duration::from_millis(500)),
                        |el, d| el.opacity(d),
                    ),
            );
        }
        col
    }

    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.p();
        let busy = !matches!(self.stage, Stage::Empty | Stage::Done);
        div()
            .w_full()
            .max_w(px(820.))
            .mx_auto()
            .px(px(48.))
            .pb_5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .pl_5()
                    .pr_2()
                    .py_2()
                    .rounded(px(28.))
                    .bg(rgb(p.raised))
                    .border_1()
                    .border_color(rgb(p.hairline))
                    .shadow_lg()
                    .child(self.round_button("attach", "+"))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(15.))
                            .text_color(rgb(p.muted))
                            .child("Ask Letronna anything"),
                    )
                    .child(
                        div()
                            .id("send")
                            .size(px(40.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(rgb(if busy { p.pill } else { BLUE }))
                            .text_color(rgb(if busy { p.on_pill } else { 0xf2f4ff }))
                            .text_size(px(16.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .active(|s| s.opacity(0.7))
                            .on_click(cx.listener(|this, _, _, cx| this.start_run(cx)))
                            .child(if busy { "■" } else { "↑" }),
                    ),
            )
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.p();
        div()
            .size_full()
            .flex()
            .font_family(SANS)
            .bg(rgb(p.page))
            .child(self.drawer(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .pt(px(28.))
                    .child(self.top_bar(cx))
                    .map(|d| {
                        if self.stage == Stage::Empty {
                            d.child(self.empty(cx))
                        } else {
                            d.child(self.chat(cx))
                        }
                    })
                    .child(self.composer(cx)),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let fonts: Vec<Cow<'static, [u8]>> = vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/Barlow_300Light.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/Barlow_400Regular.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/Barlow_500Medium.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/BodoniModa_600SemiBold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/DMMono_400Regular.ttf")),
        ];
        cx.text_system().add_fonts(fonts).unwrap();
        let dark = std::env::args().any(|a| a == "--night");
        let run = std::env::args().any(|a| a == "--run");
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.), px(780.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Letronna 1.0 preview".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(14.))),
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    let mut this = Preview {
                        dark,
                        drawer: true,
                        drawer_turn: 0,
                        stage: Stage::Empty,
                        open_steps: true,
                        run: 0,
                        selected: 0,
                    };
                    if run {
                        this.start_run(cx);
                    }
                    this
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
