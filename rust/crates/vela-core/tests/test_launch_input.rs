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
    let newest = BUILD_ALLOWED[0];
    assert_ne!(
        newest, LAUNCH,
        "the case this exists for: the newest page is not LAUNCH yet"
    );

    // A release build: refused, and nothing is set.
    assert_eq!(
        set_test_launch(Some(newest), false),
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
        set_test_launch(Some(&newest.to_ascii_uppercase()), true),
        Ok(())
    );
    assert_eq!(test_launch().as_deref(), Some(newest));
    let deployed = vec![LAUNCH.to_owned()];
    for (base, index) in [
        (OFFICIAL, None),
        (OFFICIAL, Some(deployed.as_slice())),
        (LOCAL, Some(deployed.as_slice())),
    ] {
        let chosen = target(base, index, &[], &[]).unwrap_or_else(|why| unreachable!("{why:?}"));
        assert_eq!(chosen.version(), newest, "{base} {index:?}");
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
            Some(newest),
            CheckFailure::NotChecked,
            &[],
            &[],
            false,
            1,
        );
        assert!(right.page().is_some());
    }
    // A version the person blocked stays blocked.
    let blocked = vec![newest.to_owned()];
    let chosen =
        target(OFFICIAL, None, &[], &blocked).unwrap_or_else(|why| unreachable!("{why:?}"));
    assert_eq!(chosen.version(), LAUNCH);

    // None clears it.
    assert_eq!(set_test_launch(None, true), Ok(()));
    assert_eq!(test_launch(), None);
}
