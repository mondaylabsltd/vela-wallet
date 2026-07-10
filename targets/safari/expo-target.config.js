// CANONICAL Safari target config for Vela.
// `npx create-target safari` writes targets/safari/expo-target.config.js with EMPTY
// entitlements. Copy this file over it (see docs/safari-extension/R1-INCREMENT-1-RUNBOOK.md):
//   cp packages/safari-extension/expo-target.config.template.js targets/safari/expo-target.config.js
// Kept here (version-controlled, outside the tool-owned targets/ dir) so it survives
// `create-target` regeneration and stays the source of truth.

/** @type {import('@bacons/apple-targets/app.plugin').ConfigFunction} */
module.exports = (config) => ({
  type: 'safari',
  // Reuse the app icon so prebuild's AppIcon generation needs no new art.
  // Path is relative to targets/safari/.
  icon: '../../assets/images/icon.png',
  // Keep >= the app's IPHONEOS_DEPLOYMENT_TARGET (check ios/Podfile after first prebuild).
  deploymentTarget: '15.1',
  entitlements: {
    'com.apple.security.application-groups': ['group.app.getvela.wallet'],
  },
});
