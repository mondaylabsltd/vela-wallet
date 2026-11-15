//! §38 (the R21 finding): a test build names the signing page version it
//! opens (`VELA_PAGE_LAUNCH`); a release build cannot. Its own test binary,
//! because the input is process-wide and every other `target` test assumes
//! there is none.

use vela_core::trusted_signer::integrity::{CheckFailure, BUILD_ALLOWED, LAUNCH};
use vela_core::trusted_signer::launch::{
    admit, set_test_launch, target, test_launch, TestLaunchRefused,
};

const OFFICIAL: &str = "https://sign.getvela.app/";
const LOCAL: &str = "http://localhost:5199/";

#[test]
fn a_test_build_opens_the_version_it_names_and_a_release_build_cannot() {
    // The case this exists for: a page this build accepts that is not LAUNCH
    // (a newer one, until LAUNCH moves to it; an older one once it has).
    let named = BUILD_ALLOWED
        .iter()
        .copied()
        .find(|version| *version != LAUNCH)
        .unwrap_or_else(|| unreachable!("BUILD_ALLOWED holds more than LAUNCH"));

    // A release build: refused, and nothing is set.
    assert_eq!(
        set_test_launch(Some(named), false),
        Err(TestLaunchRefused::NotATestBuild)
    );
    assert_eq!(test_launch(), None);
    // With no index the official page opens LAUNCH, as it always has.
    let plain = target(OFFICIAL, None, &[], &[]).unwrap_or_else(|why| unreachable!("{why:?}"));
    assert_eq!(plain.version(), LAUNCH);

    // Only a version this build accepts.
    for junk in ["", "  ", "nope", &"ab".repeat(32)] {
        let refused = set_test_launch(Some(junk), true);
        if junk.trim().is_empty() {
            assert_eq!(refused, Ok(()), "empty clears");
        } else {
            assert!(
                matches!(refused, Err(TestLaunchRefused::NotAccepted(_))),
                "{junk:?}"
            );
        }
        assert_eq!(test_launch(), None);
    }

    // A test build: that version, with no index, against an index that lists
    // only what production has deployed, and on a local page alike.
    assert_eq!(
        set_test_launch(Some(&named.to_ascii_uppercase()), true),
        Ok(())
    );
    assert_eq!(test_launch().as_deref(), Some(named));
    let deployed = vec![LAUNCH.to_owned()];
    for (base, index) in [
        (OFFICIAL, None),
        (OFFICIAL, Some(deployed.as_slice())),
        (LOCAL, Some(deployed.as_slice())),
    ] {
        let chosen = target(base, index, &[], &[]).unwrap_or_else(|why| unreachable!("{why:?}"));
        assert_eq!(chosen.version(), named, "{base} {index:?}");
        // …and it is still checked: other bytes are refused.
        let wrong = admit(
            &chosen,
            Some(LAUNCH),
            CheckFailure::NotChecked,
            &[],
            &[],
            false,
            1,
        );
        assert!(wrong.page().is_none());
        let right = admit(
            &chosen,
            Some(named),
            CheckFailure::NotChecked,
            &[],
            &[],
            false,
            1,
        );
        assert!(right.page().is_some());
    }
    // A version the person blocked stays blocked.
    let blocked = vec![named.to_owned()];
    let chosen =
        target(OFFICIAL, None, &[], &blocked).unwrap_or_else(|why| unreachable!("{why:?}"));
    assert_eq!(chosen.version(), LAUNCH);

    // None clears it.
    assert_eq!(set_test_launch(None, true), Ok(()));
    assert_eq!(test_launch(), None);
}
