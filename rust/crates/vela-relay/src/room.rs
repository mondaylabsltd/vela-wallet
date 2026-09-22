//! A room: at most one requester and one signer, paired and timed.
//!
//! [`Room`] holds only what the rules need — who is in which role, when the room
//! was created and when each end last sent a frame. It owns no sockets: a host
//! keeps its own handles keyed by [`ConnId`] and carries out the [`Action`]s each
//! event returns, in order.
//!
//! Every event method first applies the clock (as [`Room::on_tick`] would), so the
//! verdicts do not depend on how promptly a host's timer fires.

use crate::{Close, Control, Millis, Role, IDLE_TIMEOUT_MS, MAX_FRAME_BYTES, ROOM_LIFETIME_MS};

/// A host's handle for one WebSocket connection. Unique per connection, never
/// reused while the connection may still send events.
pub type ConnId = u64;

/// An end in the room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occupant {
    /// The host's handle.
    pub conn: ConnId,
    /// Its role.
    pub role: Role,
    /// When it last sent a frame (its connect time until it sends one).
    pub last_frame_at: Millis,
}

impl Occupant {
    /// When it goes idle.
    pub const fn idle_at(&self) -> Millis {
        self.last_frame_at.saturating_add(IDLE_TIMEOUT_MS)
    }
}

/// What a host does after an event. Apply them in the order given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Send the frame being handled — byte-for-byte, same kind (text or binary) —
    /// to `to`. Only [`Room::on_frame`] returns it.
    Forward {
        /// The receiving end.
        to: ConnId,
    },
    /// Send the relay's own text frame `control` to `to`.
    Send {
        /// The receiving end.
        to: ConnId,
        /// The frame.
        control: Control,
    },
    /// Close `conn` with `close`. The room has already let it go: later events
    /// from it are ignored.
    Close {
        /// The end to close.
        conn: ConnId,
        /// Code and reason.
        close: Close,
    },
}

/// One room's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    created_at: Millis,
    requester: Option<Occupant>,
    signer: Option<Occupant>,
}

impl Room {
    /// An empty room. `now` stands in as its creation time until its first
    /// connection, which sets it.
    pub const fn new(now: Millis) -> Self {
        Room {
            created_at: now,
            requester: None,
            signer: None,
        }
    }

    /// Rebuilds a room from what a host kept outside it — the Worker's sockets
    /// carry this across hibernation. If two occupants claim one role, the first
    /// is kept.
    pub fn restore(created_at: Millis, occupants: impl IntoIterator<Item = Occupant>) -> Self {
        let mut room = Room::new(created_at);
        for occupant in occupants {
            let slot = room.slot_mut(occupant.role);
            if slot.is_none() {
                *slot = Some(occupant);
            }
        }
        room
    }

    /// When the room was created: its first connection.
    pub const fn created_at(&self) -> Millis {
        self.created_at
    }

    /// When the room expires ([`ROOM_LIFETIME_MS`] after creation).
    pub const fn expires_at(&self) -> Millis {
        self.created_at.saturating_add(ROOM_LIFETIME_MS)
    }

    /// No one is in the room: the host forgets it.
    pub const fn is_empty(&self) -> bool {
        self.requester.is_none() && self.signer.is_none()
    }

    /// Both roles are present.
    pub const fn is_paired(&self) -> bool {
        self.requester.is_some() && self.signer.is_some()
    }

    /// The end in `role`, if any.
    pub const fn in_role(&self, role: Role) -> Option<&Occupant> {
        match role {
            Role::Requester => self.requester.as_ref(),
            Role::Signer => self.signer.as_ref(),
        }
    }

    /// The occupant with this handle, if it is one.
    pub fn occupant(&self, conn: ConnId) -> Option<&Occupant> {
        self.occupants().find(|o| o.conn == conn)
    }

    /// The ends in the room, requester first.
    pub fn occupants(&self) -> impl Iterator<Item = &Occupant> {
        self.requester.iter().chain(self.signer.iter())
    }

    /// The next time the clock alone changes something: the room's expiry or an
    /// end going idle, whichever comes first. `None` for an empty room.
    pub fn next_deadline(&self) -> Option<Millis> {
        self.occupants()
            .map(Occupant::idle_at)
            .chain((!self.is_empty()).then(|| self.expires_at()))
            .min()
    }

    /// A connection asks for `role`.
    ///
    /// A role already held refuses the newcomer with [`Close::RoleTaken`] and
    /// leaves the incumbent alone. Otherwise the newcomer is in; if that pairs
    /// the room, both ends get [`Control::Joined`] (the newcomer first). Whether
    /// `conn` got in is [`Room::occupant`]`(conn).is_some()` afterwards.
    pub fn on_connect(&mut self, role: Role, conn: ConnId, now: Millis) -> Vec<Action> {
        let mut actions = self.on_tick(now);
        if self.is_empty() {
            // A room is created by its first connection; an empty one is new.
            self.created_at = now;
        }
        if self.in_role(role).is_some() {
            actions.push(Action::Close {
                conn,
                close: Close::RoleTaken,
            });
            return actions;
        }
        *self.slot_mut(role) = Some(Occupant {
            conn,
            role,
            last_frame_at: now,
        });
        if let Some(peer) = self.in_role(role.peer()) {
            actions.push(Action::Send {
                to: conn,
                control: Control::Joined,
            });
            actions.push(Action::Send {
                to: peer.conn,
                control: Control::Joined,
            });
        }
        actions
    }

    /// `conn` sent a text or binary frame of `len` bytes.
    ///
    /// Over [`MAX_FRAME_BYTES`] closes the sender with [`Close::TooBig`] and tells
    /// its peer [`Control::Left`]. Otherwise the frame counts as activity and is
    /// forwarded to the peer — or, with no peer, dropped: the relay never buffers.
    /// A frame from a connection that is not (or no longer) in the room is
    /// dropped.
    pub fn on_frame(&mut self, conn: ConnId, len: usize, now: Millis) -> Vec<Action> {
        let mut actions = self.on_tick(now);
        let Some(role) = self.occupant(conn).map(|o| o.role) else {
            return actions;
        };
        if len > MAX_FRAME_BYTES {
            self.slot_mut(role).take();
            actions.push(Action::Close {
                conn,
                close: Close::TooBig,
            });
            self.tell_left(role, &mut actions);
            return actions;
        }
        if let Some(sender) = self.slot_mut(role) {
            sender.last_frame_at = now;
        }
        if let Some(peer) = self.in_role(role.peer()) {
            actions.push(Action::Forward { to: peer.conn });
        }
        actions
    }

    /// `conn` is gone (it closed, or its connection dropped). Its peer gets
    /// [`Control::Left`] and the room waits for a reconnect in that role. A
    /// connection that was not in the room — one refused with
    /// [`Close::RoleTaken`], say — changes nothing.
    pub fn on_leave(&mut self, conn: ConnId, now: Millis) -> Vec<Action> {
        let mut actions = self.on_tick(now);
        if let Some(role) = self.occupant(conn).map(|o| o.role) {
            self.slot_mut(role).take();
            self.tell_left(role, &mut actions);
        }
        actions
    }

    /// The clock moved to `now`. At [`Room::expires_at`] both ends are closed with
    /// [`Close::Expired`] and the room is empty. Before that, an end whose last
    /// frame is [`IDLE_TIMEOUT_MS`] old is closed with [`Close::Idle`], and its
    /// peer, if still there, gets [`Control::Left`].
    pub fn on_tick(&mut self, now: Millis) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.is_empty() {
            return actions;
        }
        if now >= self.expires_at() {
            for slot in [&mut self.requester, &mut self.signer] {
                if let Some(occupant) = slot.take() {
                    actions.push(Action::Close {
                        conn: occupant.conn,
                        close: Close::Expired,
                    });
                }
            }
            return actions;
        }
        let mut idled = Vec::new();
        for slot in [&mut self.requester, &mut self.signer] {
            if let Some(occupant) = slot.take_if(|o| now >= o.idle_at()) {
                actions.push(Action::Close {
                    conn: occupant.conn,
                    close: Close::Idle,
                });
                idled.push(occupant.role);
            }
        }
        for role in idled {
            self.tell_left(role, &mut actions);
        }
        actions
    }

    fn slot_mut(&mut self, role: Role) -> &mut Option<Occupant> {
        match role {
            Role::Requester => &mut self.requester,
            Role::Signer => &mut self.signer,
        }
    }

    /// The end in `role` just went: tell its peer, if there is one.
    fn tell_left(&self, role: Role, actions: &mut Vec<Action>) {
        if let Some(peer) = self.in_role(role.peer()) {
            actions.push(Action::Send {
                to: peer.conn,
                control: Control::Left,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::{Close as Shut, Forward, Send};
    use Control::{Joined, Left};
    use Role::{Requester, Signer};

    const T0: Millis = 1_000_000;
    const R: ConnId = 1;
    const S: ConnId = 2;

    /// A room with the requester in at `T0` and the signer in at `T0 + 1`.
    fn paired() -> Room {
        let mut room = Room::new(T0);
        assert_eq!(room.on_connect(Requester, R, T0), vec![]);
        room.on_connect(Signer, S, T0 + 1);
        assert!(room.is_paired());
        room
    }

    #[test]
    fn first_connection_creates_the_room_and_waits() {
        let mut room = Room::new(0);
        assert!(room.is_empty());
        assert_eq!(room.next_deadline(), None);
        assert_eq!(room.on_connect(Requester, R, T0), vec![]);
        assert_eq!(room.created_at(), T0);
        assert!(!room.is_paired());
        assert_eq!(room.occupant(R).map(|o| o.role), Some(Requester));
    }

    #[test]
    fn second_role_pairs_and_both_are_told_newcomer_first() {
        let mut room = Room::new(T0);
        room.on_connect(Signer, S, T0);
        assert_eq!(
            room.on_connect(Requester, R, T0 + 5),
            vec![
                Send {
                    to: R,
                    control: Joined
                },
                Send {
                    to: S,
                    control: Joined
                }
            ]
        );
        assert!(room.is_paired());
        // Pairing does not restart the room's clock.
        assert_eq!(room.created_at(), T0);
    }

    #[test]
    fn a_taken_role_refuses_the_newcomer_and_leaves_the_incumbent_alone() {
        let mut room = paired();
        let before = room.clone();
        let third: ConnId = 3;
        assert_eq!(
            room.on_connect(Signer, third, T0 + 10),
            vec![Shut {
                conn: third,
                close: Close::RoleTaken
            }]
        );
        assert_eq!(room, before);
        assert!(room.occupant(third).is_none());
        assert_eq!(room.in_role(Signer).map(|o| o.conn), Some(S));
        // The refused connection's frames and its leaving change nothing.
        assert_eq!(room.on_frame(third, 10, T0 + 11), vec![]);
        assert_eq!(room.on_leave(third, T0 + 12), vec![]);
        assert_eq!(room, before);
        // And the incumbent still forwards.
        assert_eq!(room.on_frame(S, 10, T0 + 13), vec![Forward { to: R }]);
    }

    #[test]
    fn a_taken_role_is_refused_even_before_pairing() {
        let mut room = Room::new(T0);
        room.on_connect(Requester, R, T0);
        assert_eq!(
            room.on_connect(Requester, 9, T0 + 1),
            vec![Shut {
                conn: 9,
                close: Close::RoleTaken
            }]
        );
        assert_eq!(room.in_role(Requester).map(|o| o.conn), Some(R));
    }

    #[test]
    fn frames_forward_to_the_peer_both_ways() {
        let mut room = paired();
        assert_eq!(room.on_frame(R, 100, T0 + 2), vec![Forward { to: S }]);
        assert_eq!(room.on_frame(S, 100, T0 + 3), vec![Forward { to: R }]);
        // Empty frames are frames.
        assert_eq!(room.on_frame(R, 0, T0 + 4), vec![Forward { to: S }]);
    }

    #[test]
    fn no_buffering_a_frame_with_no_peer_is_dropped() {
        let mut room = Room::new(T0);
        room.on_connect(Requester, R, T0);
        assert_eq!(room.on_frame(R, 10, T0 + 1), vec![]);
        // Nothing is held for the signer: joining yields only the joined frames.
        assert_eq!(
            room.on_connect(Signer, S, T0 + 2),
            vec![
                Send {
                    to: S,
                    control: Joined
                },
                Send {
                    to: R,
                    control: Joined
                }
            ]
        );
    }

    #[test]
    fn a_frame_at_the_limit_forwards_and_one_byte_over_closes_the_sender() {
        let mut room = paired();
        assert_eq!(
            room.on_frame(S, MAX_FRAME_BYTES, T0 + 2),
            vec![Forward { to: R }]
        );
        assert_eq!(
            room.on_frame(S, MAX_FRAME_BYTES + 1, T0 + 3),
            vec![
                Shut {
                    conn: S,
                    close: Close::TooBig
                },
                Send {
                    to: R,
                    control: Left
                }
            ]
        );
        assert!(room.occupant(S).is_none());
        assert!(room.occupant(R).is_some());
        // The closed end's later frames and leave are ignored.
        assert_eq!(room.on_frame(S, 1, T0 + 4), vec![]);
        assert_eq!(room.on_leave(S, T0 + 5), vec![]);
    }

    #[test]
    fn an_oversize_frame_with_no_peer_closes_only_the_sender() {
        let mut room = Room::new(T0);
        room.on_connect(Signer, S, T0);
        assert_eq!(
            room.on_frame(S, usize::MAX, T0 + 1),
            vec![Shut {
                conn: S,
                close: Close::TooBig
            }]
        );
        assert!(room.is_empty());
    }

    #[test]
    fn leaving_tells_the_peer_and_the_role_can_be_filled_again() {
        let mut room = paired();
        assert_eq!(
            room.on_leave(S, T0 + 2),
            vec![Send {
                to: R,
                control: Left
            }]
        );
        assert!(!room.is_paired());
        // A frame while the peer is away is dropped.
        assert_eq!(room.on_frame(R, 5, T0 + 3), vec![]);
        let s2: ConnId = 7;
        assert_eq!(
            room.on_connect(Signer, s2, T0 + 4),
            vec![
                Send {
                    to: s2,
                    control: Joined
                },
                Send {
                    to: R,
                    control: Joined
                }
            ]
        );
        assert_eq!(room.on_frame(R, 5, T0 + 5), vec![Forward { to: s2 }]);
        // The requester can reconnect too.
        assert_eq!(
            room.on_leave(R, T0 + 6),
            vec![Send {
                to: s2,
                control: Left
            }]
        );
        assert_eq!(
            room.on_connect(Requester, 8, T0 + 7),
            vec![
                Send {
                    to: 8,
                    control: Joined
                },
                Send {
                    to: s2,
                    control: Joined
                }
            ]
        );
        // A reconnect does not extend the room's life.
        assert_eq!(room.created_at(), T0);
    }

    #[test]
    fn the_last_one_out_empties_the_room() {
        let mut room = paired();
        room.on_leave(R, T0 + 2);
        assert_eq!(room.on_leave(S, T0 + 3), vec![]);
        assert!(room.is_empty());
        assert_eq!(room.next_deadline(), None);
        assert_eq!(room.on_tick(T0 + ROOM_LIFETIME_MS * 10), vec![]);
    }

    #[test]
    fn an_empty_room_is_created_afresh_by_its_next_connection() {
        let mut room = paired();
        room.on_leave(R, T0 + 2);
        room.on_leave(S, T0 + 3);
        let later = T0 + 5 * 60 * 1000;
        room.on_connect(Signer, 9, later);
        assert_eq!(room.created_at(), later);
        assert_eq!(room.expires_at(), later + ROOM_LIFETIME_MS);
    }

    #[test]
    fn the_room_expires_ten_minutes_after_creation_for_both_ends() {
        let mut room = paired();
        // Keep both ends busy so idleness never applies.
        let mut t = T0;
        while t + 60_000 < T0 + ROOM_LIFETIME_MS {
            t += 60_000;
            room.on_frame(R, 1, t);
            room.on_frame(S, 1, t);
        }
        assert_eq!(room.on_tick(T0 + ROOM_LIFETIME_MS - 1), vec![]);
        assert_eq!(room.next_deadline(), Some(T0 + ROOM_LIFETIME_MS));
        assert_eq!(
            room.on_tick(T0 + ROOM_LIFETIME_MS),
            vec![
                Shut {
                    conn: R,
                    close: Close::Expired
                },
                Shut {
                    conn: S,
                    close: Close::Expired
                }
            ]
        );
        assert!(room.is_empty());
    }

    #[test]
    fn expiry_counts_from_the_first_connection_not_the_pairing() {
        let mut room = Room::new(T0);
        room.on_connect(Requester, R, T0);
        let late = T0 + ROOM_LIFETIME_MS - 1000;
        // Stay active until the signer arrives.
        let mut t = T0;
        while t + 100_000 < late {
            t += 100_000;
            room.on_frame(R, 1, t);
        }
        room.on_connect(Signer, S, late);
        assert_eq!(room.next_deadline(), Some(T0 + ROOM_LIFETIME_MS));
        assert_eq!(room.on_tick(T0 + ROOM_LIFETIME_MS).len(), 2);
        assert!(room.is_empty());
    }

    #[test]
    fn an_idle_end_is_closed_and_its_peer_told() {
        let mut room = paired();
        // The signer keeps talking; the requester never does.
        room.on_frame(S, 1, T0 + 100_000);
        assert_eq!(room.on_tick(T0 + IDLE_TIMEOUT_MS - 1), vec![]);
        assert_eq!(room.next_deadline(), Some(T0 + IDLE_TIMEOUT_MS));
        assert_eq!(
            room.on_tick(T0 + IDLE_TIMEOUT_MS),
            vec![
                Shut {
                    conn: R,
                    close: Close::Idle
                },
                Send {
                    to: S,
                    control: Left
                }
            ]
        );
        assert!(room.occupant(R).is_none());
        assert!(room.occupant(S).is_some());
        assert_eq!(room.next_deadline(), Some(T0 + 100_000 + IDLE_TIMEOUT_MS));
    }

    #[test]
    fn an_end_waiting_alone_idles_too() {
        let mut room = Room::new(T0);
        room.on_connect(Requester, R, T0);
        assert_eq!(
            room.on_tick(T0 + IDLE_TIMEOUT_MS),
            vec![Shut {
                conn: R,
                close: Close::Idle
            }]
        );
        assert!(room.is_empty());
    }

    #[test]
    fn a_frame_resets_the_idle_clock() {
        let mut room = paired();
        let t = T0 + IDLE_TIMEOUT_MS - 1;
        room.on_frame(R, 1, t);
        room.on_frame(S, 1, t);
        assert_eq!(room.on_tick(T0 + IDLE_TIMEOUT_MS + 1), vec![]);
        assert_eq!(room.next_deadline(), Some(t + IDLE_TIMEOUT_MS));
    }

    #[test]
    fn an_oversize_frame_does_not_count_as_activity_for_anyone() {
        let mut room = paired();
        room.on_frame(S, MAX_FRAME_BYTES + 1, T0 + 100_000);
        // The requester's clock is untouched by the signer's refused frame.
        assert_eq!(room.next_deadline(), Some(T0 + IDLE_TIMEOUT_MS));
    }

    #[test]
    fn both_idle_are_both_closed_with_no_left() {
        let mut room = paired();
        assert_eq!(
            room.on_tick(T0 + 1 + IDLE_TIMEOUT_MS),
            vec![
                Shut {
                    conn: R,
                    close: Close::Idle
                },
                Shut {
                    conn: S,
                    close: Close::Idle
                }
            ]
        );
        assert!(room.is_empty());
    }

    #[test]
    fn a_frame_arriving_after_the_idle_deadline_is_too_late() {
        // The host's timer has not fired yet; the verdict is the same.
        let mut room = paired();
        room.on_frame(S, 1, T0 + 100_000);
        assert_eq!(
            room.on_frame(R, 1, T0 + IDLE_TIMEOUT_MS),
            vec![
                Shut {
                    conn: R,
                    close: Close::Idle
                },
                Send {
                    to: S,
                    control: Left
                }
            ]
        );
        assert!(room.occupant(R).is_none());
    }

    #[test]
    fn a_connect_to_an_expired_room_closes_the_old_ends_and_starts_afresh() {
        let mut room = paired();
        let late = T0 + ROOM_LIFETIME_MS + 5;
        let actions = room.on_connect(Requester, 9, late);
        assert_eq!(
            actions,
            vec![
                Shut {
                    conn: R,
                    close: Close::Expired
                },
                Shut {
                    conn: S,
                    close: Close::Expired
                }
            ]
        );
        assert_eq!(room.created_at(), late);
        assert_eq!(room.in_role(Requester).map(|o| o.conn), Some(9));
    }

    #[test]
    fn an_idle_incumbent_frees_its_role_for_the_newcomer() {
        let mut room = paired();
        room.on_frame(S, 1, T0 + 100_000);
        let t = T0 + IDLE_TIMEOUT_MS;
        assert_eq!(
            room.on_connect(Requester, 9, t),
            vec![
                Shut {
                    conn: R,
                    close: Close::Idle
                },
                Send {
                    to: S,
                    control: Left
                },
                Send {
                    to: 9,
                    control: Joined
                },
                Send {
                    to: S,
                    control: Joined
                },
            ]
        );
    }

    #[test]
    fn a_refused_newcomer_still_gets_the_clock_applied_first() {
        // The signer idles while the requester is active; a second requester is
        // refused, but the signer's idle close still happens.
        let mut room = paired();
        room.on_frame(R, 1, T0 + 100_000);
        let t = T0 + 1 + IDLE_TIMEOUT_MS;
        assert_eq!(
            room.on_connect(Requester, 9, t),
            vec![
                Shut {
                    conn: S,
                    close: Close::Idle
                },
                Send {
                    to: R,
                    control: Left
                },
                Shut {
                    conn: 9,
                    close: Close::RoleTaken
                },
            ]
        );
    }

    #[test]
    fn next_deadline_is_the_earliest_of_expiry_and_idleness() {
        let mut room = Room::new(T0);
        room.on_connect(Requester, R, T0);
        assert_eq!(room.next_deadline(), Some(T0 + IDLE_TIMEOUT_MS));
        room.on_connect(Signer, S, T0 + 50_000);
        assert_eq!(room.next_deadline(), Some(T0 + IDLE_TIMEOUT_MS));
        room.on_frame(R, 1, T0 + 60_000);
        assert_eq!(room.next_deadline(), Some(T0 + 50_000 + IDLE_TIMEOUT_MS));
        // Close to the end of its life, expiry comes first.
        let mut t = T0 + 60_000;
        while t + 100_000 < T0 + ROOM_LIFETIME_MS {
            t += 100_000;
            room.on_frame(R, 1, t);
            room.on_frame(S, 1, t);
        }
        assert_eq!(room.next_deadline(), Some(T0 + ROOM_LIFETIME_MS));
    }

    #[test]
    fn restore_round_trips_and_keeps_the_first_claim_to_a_role() {
        let room = paired();
        let copy = Room::restore(room.created_at(), room.occupants().copied());
        assert_eq!(copy, room);

        let a = Occupant {
            conn: 1,
            role: Signer,
            last_frame_at: T0,
        };
        let b = Occupant {
            conn: 2,
            role: Signer,
            last_frame_at: T0,
        };
        let room = Room::restore(T0, [a, b]);
        assert_eq!(room.in_role(Signer), Some(&a));
        assert!(room.in_role(Requester).is_none());
    }

    #[test]
    fn a_restored_room_applies_the_same_rules() {
        // What the Worker does on every event: rebuild, apply, persist.
        let mut room = paired();
        room.on_frame(R, 1, T0 + 30_000);
        let snapshot: Vec<Occupant> = room.occupants().copied().collect();
        let mut restored = Room::restore(room.created_at(), snapshot);
        assert_eq!(
            restored.on_tick(T0 + 1 + IDLE_TIMEOUT_MS),
            room.on_tick(T0 + 1 + IDLE_TIMEOUT_MS)
        );
        assert_eq!(restored, room);
    }
}
