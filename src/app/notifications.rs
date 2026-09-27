//! Chiming when a session changes state.
//!
//! Seventeen places across the app assign `session.status`, so this watches the
//! result rather than instrumenting every writer: one pass at the end of the
//! event drain compares each session's status against what was last announced
//! and chimes on the edges that deserve it. A status that changes and changes
//! back within a single drain is therefore silent, which is the right answer —
//! nothing the user could have noticed actually happened.
//!
//! Desktop banners stay where Waku already posts them, on turn completion; this
//! only adds the sound and routes both through the user's preferences.

use std::collections::HashMap;

use uuid::Uuid;
use anastasia_protocol::model::SessionStatus;

use super::*;

impl Waku {
    /// Chime for any session whose status moved since the last drain.
    pub(super) fn announce_status_transitions(&mut self, cx: &mut Context<Self>) {
        let mut current: HashMap<Uuid, SessionStatus> =
            HashMap::with_capacity(self.state.sessions.len());
        for session in &self.state.sessions {
            current.insert(session.id, session.status);
        }

        // First pass after launch: adopt the current statuses without chiming.
        // Restoring a transcript is not an event the user needs announcing.
        if self.announced_statuses.is_empty() {
            self.announced_statuses = current;
            return;
        }

        let announce = self.state.notifications.sound_enabled
            && self
                .state
                .notifications
                .should_announce(cx.active_window().is_some());

        for (session_id, status) in &current {
            let previous = self.announced_statuses.get(session_id).copied();
            // A session that appeared this drain has no transition to report.
            let Some(previous) = previous else {
                continue;
            };
            if announce && let Some(sound) = crate::sound::sound_for_transition(previous, *status) {
                crate::sound::play(sound);
            }
        }

        // Replace wholesale so sessions that were closed stop being tracked.
        self.announced_statuses = current;
    }
}
