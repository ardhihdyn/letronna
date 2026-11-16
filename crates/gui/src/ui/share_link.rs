//! Sharing a session by link from its menu (Share link, Copy shared link,
//! Stop sharing), and bringing a shared session in from a link on the
//! clipboard (the Import page). What is shared, and the calls, are core's
//! `share`. Removing this file, its `mod` line, its menu items and the
//! Import page's button removes it from the app.

use crate::prelude::*;
use gpui::PromptLevel;
use letronna_core::share;

impl Chat {
    /// Share the session at `index` by link, once asked whether to; the link
    /// is copied. Shared again, the same session's link shows it as it is now.
    pub(crate) fn share_session(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fill_now(index);
        let c = &self.store.conversations[index];
        let (session, title, messages) = (c.id, c.title.clone(), c.messages.clone());
        let again = share::shared_as(session).is_some();
        let asked = window.prompt(
            PromptLevel::Info,
            if again {
                "Update the shared link?"
            } else {
                "Share this session by link?"
            },
            Some(
                "Anyone with the link can read what you asked and each reply, and bring it \
                 into Letronna. Commands, file contents and other steps are not shared. You \
                 can stop sharing at any time from this menu.",
            ),
            &[if again { "Update link" } else { "Share" }, "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if asked.await != Ok(0) {
                return;
            }
            let made = cx
                .background_executor()
                .spawn(async move { share::share(session, &title, &messages) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match made {
                    Ok(link) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(link));
                        this.toast(
                            "Link copied: Anyone with it can read this session.",
                            crate::ui::toast::Kind::Info,
                            cx,
                        );
                    }
                    Err(why) => {
                        this.toast(format!("Share: {why}"), crate::ui::toast::Kind::Error, cx)
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn copy_shared_link(
        &mut self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(link) = share::shared_as(self.store.conversations[index].id) {
            cx.write_to_clipboard(ClipboardItem::new_string(link));
            self.toast("Link copied.", crate::ui::toast::Kind::Info, cx);
        }
    }

    /// Stop sharing the session at `index`: its link stops working.
    pub(crate) fn stop_sharing(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.store.conversations[index].id;
        cx.spawn_in(window, async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move { share::unshare(session) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match done {
                    Ok(()) => this.toast(
                        "Stopped sharing: The link no longer opens this session.",
                        crate::ui::toast::Kind::Info,
                        cx,
                    ),
                    Err(why) => {
                        this.toast(format!("Share: {why}"), crate::ui::toast::Kind::Error, cx)
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Bring in the shared session whose link is on the clipboard, as a new
    /// session, and open it.
    pub(crate) fn open_shared_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pasted = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        if share::id_of(&pasted).is_none() {
            self.toast(
                "Import: Copy a shared session's link (letronna.ai/s/…) first, then click this again.",
                crate::ui::toast::Kind::Info,
                cx,
            );
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let opened = cx
                .background_executor()
                .spawn(async move { share::open(&pasted) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match opened {
                Ok(shared) => {
                    let id = this.store.reserve_id();
                    this.store
                        .conversations
                        .push(letronna_core::store::Conversation {
                            id,
                            title: shared.title.clone(),
                            messages: share::messages_of(&shared),
                            updated: letronna_core::chat::now(),
                            ..Default::default()
                        });
                    let index = this.store.conversations.len() - 1;
                    this.open_imported(index, window, cx);
                }
                Err(why) => this.toast(format!("Import: {why}"), crate::ui::toast::Kind::Error, cx),
            });
        })
        .detach();
    }
}
