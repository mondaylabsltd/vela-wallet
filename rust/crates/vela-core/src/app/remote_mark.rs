//! How long a remote logo that failed to load stays failed (spec 082, RE10).
//!
//! Pure. A miss is classed from its HTTP status: a missing or refused image
//! is remembered for the session, a throttled, broken or unreached one for a
//! short while, so a logo lost to a bad minute comes back. Filled in by
//! EF-core (T035); registered early (T009) so no later group edits
//! `app/mod.rs`.
