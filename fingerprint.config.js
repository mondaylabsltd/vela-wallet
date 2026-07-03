// Read by @expo/fingerprint (runtimeVersion policy "fingerprint").
//
// app.config.js injects the current git commit into `extra.gitCommit` (for the
// in-app build-info screen). Without this skip, that makes the fingerprint —
// and therefore the update runtime — unique PER COMMIT, so no PR ever matches
// an existing dev build and the preview workflow starts a fresh build for
// every PR instead of just publishing an update.
/** @type {import('@expo/fingerprint').Config} */
const { SourceSkips } = require('@expo/fingerprint');

module.exports = {
  sourceSkips: SourceSkips.ExpoConfigExtraSection,
};
