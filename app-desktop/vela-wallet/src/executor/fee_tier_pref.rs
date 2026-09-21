//! The default transaction speed's two operations (spec 068; the desktop's
//! since 069): read the stored name, write a chosen one.
//!
//! The same two sentences the display currency speaks, against the same
//! store, so there is one way a committed preference reaches disk here. The
//! stored value goes back to the core RAW — whether a string is a tier is
//! `fee_tier_pref::parse_stored`'s call, so a name this build does not know
//! reads as "never chose" (the factory `fast`) instead of being coerced into
//! something that would then go on the wire.
//!
//! `vela.feeTier` survives sign-out, which clears only the accounts and the
//! active index: a speed preference belongs to the person and the device.

use gpui::App;

use vela_core::app::fee_tier_pref::{
    Event, FeeTierPref, FeeTierPrefOperation, FeeTierPrefShellResult,
};

use crate::executor::storage;
use crate::resident::{Answer, Machine};

impl Machine for FeeTierPref {
    const LABEL: &'static str = "fee_tier_pref";

    fn boot_event(_cx: &App) -> Event {
        Event::Refresh
    }

    fn perform(operation: &FeeTierPrefOperation) -> Answer<FeeTierPrefShellResult, Self::Event> {
        match operation {
            // Absent ALWAYS means "never chose" — including a read that
            // failed, which is why this cannot surface an error.
            FeeTierPrefOperation::ReadStoredTier => {
                Answer::Now(FeeTierPrefShellResult::StoredTier {
                    raw: storage::read_value(storage::KEY_FEE_TIER)
                        .ok()
                        .flatten()
                        .as_ref()
                        .and_then(|value| value.as_str())
                        .map(str::to_owned),
                })
            }
            // Best effort: the committed choice stays on screen either way,
            // and the next launch simply reads the old value.
            FeeTierPrefOperation::WriteStoredTier { tier } => {
                let _ = storage::write_value(
                    storage::KEY_FEE_TIER,
                    serde_json::Value::String(tier.clone()),
                );
                Answer::Now(FeeTierPrefShellResult::TierWritten)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use crate::executor::storage::tests::with_temp_state;
    use vela_core::app::fee_policy::FeeTier;
    use vela_core::app::fee_tier_pref::FeeTierPrefView;

    fn drive(host: &mut CoreHost<FeeTierPref>, event: Event) -> FeeTierPrefView {
        let mut pending = host.dispatch(event);
        while let Some(next) = pending.pop() {
            let Answer::Now(result) = FeeTierPref::perform(&next.operation) else {
                unreachable!("every fee_tier_pref operation is local");
            };
            pending.extend(host.resolve(next.id, result));
        }
        host.view()
    }

    /// A fresh install starts where every send started before there was a
    /// choice, and says nobody chose it.
    #[test]
    fn a_fresh_install_is_the_factory_fast() {
        with_temp_state("fee-tier-fresh", || {
            let mut host = CoreHost::<FeeTierPref>::new();
            let view = drive(&mut host, Event::Refresh);
            assert_eq!(view.tier, FeeTier::Fast);
            assert!(!view.committed);
        });
    }

    /// A choice is written as the bare name the other clients store, and a
    /// new machine (the next launch) reads it back.
    #[test]
    fn a_choice_persists_across_launches() {
        with_temp_state("fee-tier-persist", || {
            let mut host = CoreHost::<FeeTierPref>::new();
            drive(&mut host, Event::Refresh);
            drive(
                &mut host,
                Event::UserChose {
                    tier: FeeTier::Slow,
                },
            );
            assert_eq!(
                storage::read_value(storage::KEY_FEE_TIER).ok().flatten(),
                Some(serde_json::Value::String("slow".to_owned()))
            );
            let mut next = CoreHost::<FeeTierPref>::new();
            let view = drive(&mut next, Event::Refresh);
            assert_eq!(view.tier, FeeTier::Slow);
            assert!(view.committed);
        });
    }

    /// A value this build does not offer is not a tier: it reads as the
    /// factory default and is left alone on disk.
    #[test]
    fn an_unknown_name_reads_as_unset() {
        with_temp_state("fee-tier-unknown", || {
            let _ = storage::write_value(
                storage::KEY_FEE_TIER,
                serde_json::Value::String("rapid".to_owned()),
            );
            let mut host = CoreHost::<FeeTierPref>::new();
            let view = drive(&mut host, Event::Refresh);
            assert_eq!(view.tier, FeeTier::Fast);
            assert!(!view.committed);
            assert_eq!(
                storage::read_value(storage::KEY_FEE_TIER).ok().flatten(),
                Some(serde_json::Value::String("rapid".to_owned()))
            );
        });
    }
}
