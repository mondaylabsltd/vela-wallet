//! Whether the network is there, from the answers the app already gets (spec 082, RE3).
//!
//! Pure. Each read either reached a node or did not; a run of misses is
//! "went offline", the first answer after it is "came back", and the shells
//! retry what failed on that edge. Android's `NetHealth.kt` moves here so
//! every shell counts the same way. Filled in by EF-core (T034); registered
//! early (T009) so no later group edits `app/mod.rs`.
