const { withAppBuildGradle } = require('@expo/config-plugins');

// ---------------------------------------------------------------------------
// Android release signing (durable across `expo prebuild --clean`).
//
// The generated android/ project is gitignored, so editing its build.gradle by
// hand does not survive a prebuild. This plugin patches the generated
// android/app/build.gradle to sign release builds with the upload keystore
// described by android/keystore.properties (see keystore.properties.example at
// the repo root). Without that file the release build falls back to the DEBUG
// keystore and logs a loud warning — fine for local `expo run:android`, but the
// output must never be published: Play re-signs the app and passkey Digital
// Asset Links verification fails unless assetlinks.json carries the real cert
// fingerprints (docs/NATIVE-LAUNCH-CHECKLIST.md A1).
// ---------------------------------------------------------------------------

const PROPS_SNIPPET = `
    def keystorePropertiesFile = rootProject.file('keystore.properties')
    def keystoreProperties = new Properties()
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.withInputStream { keystoreProperties.load(it) }
    }
`;

const RELEASE_CONFIG_SNIPPET = `        if (keystorePropertiesFile.exists()) {
            release {
                storeFile rootProject.file(keystoreProperties['storeFile'])
                storePassword keystoreProperties['storePassword']
                keyAlias keystoreProperties['keyAlias']
                keyPassword keystoreProperties['keyPassword']
            }
        }
`;

const RELEASE_BUILDTYPE_SNIPPET = `            if (keystorePropertiesFile.exists()) {
                signingConfig signingConfigs.release
            } else {
                logger.warn('WARNING: android/keystore.properties not found — release build ' +
                        'is signed with the DEBUG keystore and must not be published ' +
                        '(passkey Digital Asset Links will fail). See keystore.properties.example.')
                signingConfig signingConfigs.debug
            }
`;

function patchBuildGradle(contents) {
  if (contents.includes('keystorePropertiesFile')) return contents; // already patched

  // 1. Load keystore.properties just before the signingConfigs block.
  contents = contents.replace(
    /(\n\s*signingConfigs \{)/,
    `\n${PROPS_SNIPPET}$1`,
  );

  // 2. Add the conditional release signingConfig after the debug one.
  contents = contents.replace(
    /(signingConfigs \{[\s\S]*?debug \{[\s\S]*?\n\s{8}\})/,
    `$1\n${RELEASE_CONFIG_SNIPPET}`,
  );

  // 3. Replace the template's `signingConfig signingConfigs.debug` inside the
  //    release build type (keep the debug buildType's line intact).
  contents = contents.replace(
    /(release \{\s*\n)(\s*\/\/ Caution![^\n]*\n\s*\/\/ see[^\n]*\n)?\s*signingConfig signingConfigs\.debug\n/,
    `$1${RELEASE_BUILDTYPE_SNIPPET}`,
  );

  return contents;
}

module.exports = function withReleaseSigning(config) {
  return withAppBuildGradle(config, (mod) => {
    if (mod.modResults.language !== 'groovy') {
      throw new Error('with-release-signing: expected a groovy build.gradle');
    }
    mod.modResults.contents = patchBuildGradle(mod.modResults.contents);
    return mod;
  });
};
