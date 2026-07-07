const {
  withInfoPlist,
  withEntitlementsPlist,
  withAndroidManifest,
  withDangerousMod,
  withXcodeProject,
} = require('@expo/config-plugins');
const fs = require('fs');
const path = require('path');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function copyFilesRecursive(srcDir, destDir) {
  if (!fs.existsSync(srcDir)) return;
  fs.mkdirSync(destDir, { recursive: true });
  for (const entry of fs.readdirSync(srcDir, { withFileTypes: true })) {
    const src = path.join(srcDir, entry.name);
    const dest = path.join(destDir, entry.name);
    if (entry.isDirectory()) {
      copyFilesRecursive(src, dest);
    } else {
      fs.copyFileSync(src, dest);
    }
  }
}

// ---------------------------------------------------------------------------
// iOS – Info.plist entries
// ---------------------------------------------------------------------------

function withIOSInfoPlist(config) {
  return withInfoPlist(config, (mod) => {
    mod.modResults.NSCameraUsageDescription =
      mod.modResults.NSCameraUsageDescription ||
      'Vela Wallet uses the camera to scan QR codes for wallet addresses.';

    // The wallet does not record audio. expo-camera adds a microphone usage
    // string by default; strip it so the App Store doesn't flag an unused
    // permission (this plugin runs after expo-camera in the plugins array).
    delete mod.modResults.NSMicrophoneUsageDescription;

    return mod;
  });
}

// ---------------------------------------------------------------------------
// iOS – Entitlements (Associated Domains for passkeys)
// ---------------------------------------------------------------------------

function withIOSEntitlements(config) {
  return withEntitlementsPlist(config, (mod) => {
    // NOTE: the iCloud Key-Value Store entitlement was removed. No JS consumes
    // it (the vela-cloud-sync module is not wired), and emitting an entitlement
    // the provisioning profile doesn't grant fails Release codesign/Archive.
    // Re-add it here AND enable the iCloud KV capability on the App ID together,
    // only when cloud sync is actually shipped.

    // Associated Domains: passkeys (webcredentials) + Universal Links (applinks).
    // Both resolve against getvela.app's AASA. applinks powers the Safari
    // extension's one-tap sign hand-off (https://getvela.app/sign?rid=…) — the
    // extension only USES it once the app has attested the association resolves on
    // this device (see app-group-account-sync.ts), so shipping the entitlement is
    // harmless (the capability is already granted for webcredentials).
    //
    // DEV BYPASS: on iOS ≥14 devices fetch the AASA from Apple's CDN, which can lag
    // hours. Set VELA_AASA_DEV_MODE=1 at prebuild to emit `applinks:getvela.app?mode=developer`
    // instead — with iPhone Settings › Developer › Associated Domains Development ON,
    // swcd fetches getvela.app directly, so a server AASA edit is live immediately.
    // Env-gated so a normal/distribution build NEVER carries ?mode=developer (which
    // distribution ignores anyway, but must not linger as an unvalidated path).
    if (!Array.isArray(mod.modResults['com.apple.developer.associated-domains'])) {
      mod.modResults['com.apple.developer.associated-domains'] = [];
    }
    const domains = mod.modResults['com.apple.developer.associated-domains'];
    const applinks =
      process.env.VELA_AASA_DEV_MODE === '1' ? 'applinks:getvela.app?mode=developer' : 'applinks:getvela.app';
    for (const d of ['webcredentials:getvela.app', applinks]) {
      if (!domains.includes(d)) domains.push(d);
    }

    // App Group shared with the Safari Web Extension target (Safari R1 spike).
    // The extension target declares the SAME group in targets/safari/expo-target.config.js.
    // Both App IDs (app.getvela.VelaWallet + app.getvela.VelaWallet.safari) must enable
    // App Groups in the Apple Developer portal, or Release codesign fails — same class of
    // warning as the iCloud-KV note above.
    if (!Array.isArray(mod.modResults['com.apple.security.application-groups'])) {
      mod.modResults['com.apple.security.application-groups'] = ['group.app.getvela.wallet'];
    } else if (
      !mod.modResults['com.apple.security.application-groups'].includes('group.app.getvela.wallet')
    ) {
      mod.modResults['com.apple.security.application-groups'].push('group.app.getvela.wallet');
    }

    return mod;
  });
}

// ---------------------------------------------------------------------------
// iOS – Copy Swift / ObjC source files into the Xcode project
// ---------------------------------------------------------------------------

function withIOSSourceFiles(config) {
  return withDangerousMod(config, [
    'ios',
    (mod) => {
      const projectRoot = mod.modRequest.projectRoot;
      const projectName = mod.modRequest.projectName || 'velawallet';
      // Copy directly into the project root so Xcode compiles them automatically.
      // Subdirectories under ios/<project>/ are NOT auto-included in the build.
      const destDir = path.join(projectRoot, 'ios', projectName);

      // vela-cloud-sync is intentionally omitted — it has no JS consumer yet.
      // vela-app-group: App Group shared-container IPC (Increment 2 Safari spike).
      // vela-wallet-webview: the in-app dApp browser's WKWebView native view.
      const modules = ['vela-passkey', 'vela-app-group', 'vela-wallet-webview'];
      for (const moduleName of modules) {
        const srcDir = path.join(projectRoot, 'modules', moduleName, 'ios');
        if (!fs.existsSync(srcDir)) continue;
        for (const file of fs.readdirSync(srcDir)) {
          fs.copyFileSync(
            path.join(srcDir, file),
            path.join(destDir, file),
          );
        }
      }

      // Patch the bridging header so Swift can see React Native types
      const bridgingHeader = path.join(destDir, `${projectName}-Bridging-Header.h`);
      if (fs.existsSync(bridgingHeader)) {
        let content = fs.readFileSync(bridgingHeader, 'utf8');
        const requiredImports = [
          '#import <React/RCTBridgeModule.h>',
          '#import <React/RCTEventEmitter.h>',
          '#import <React/RCTViewManager.h>',
        ];
        for (const imp of requiredImports) {
          if (!content.includes(imp)) {
            content += `\n${imp}`;
          }
        }
        fs.writeFileSync(bridgingHeader, content, 'utf8');
      }

      return mod;
    },
  ]);
}

// ---------------------------------------------------------------------------
// Android – Permissions in AndroidManifest.xml
// ---------------------------------------------------------------------------

function withAndroidPermissions(config) {
  return withAndroidManifest(config, (mod) => {
    const manifest = mod.modResults.manifest;

    const requiredPermissions = [
      'android.permission.CAMERA',
    ];

    // Permissions pulled in by dependencies that this wallet does NOT use:
    //   - RECORD_AUDIO: added by expo-camera (no audio/video recording here).
    //   - SYSTEM_ALERT_WINDOW: added by the dev menu overlay.
    // We emit `tools:node="remove"` so they are stripped from the final merged
    // manifest regardless of which library contributed them. (This plugin runs
    // last in app.json's plugins array, so it also wins over earlier plugins.)
    const removePermissions = [
      'android.permission.RECORD_AUDIO',
      'android.permission.SYSTEM_ALERT_WINDOW',
    ];

    if (!Array.isArray(manifest['uses-permission'])) {
      manifest['uses-permission'] = [];
    }

    // Drop any existing plain declarations for permissions we manage, so each
    // appears exactly once in its canonical form below.
    const managed = new Set([...requiredPermissions, ...removePermissions]);
    manifest['uses-permission'] = manifest['uses-permission'].filter(
      (p) => !managed.has(p.$?.['android:name'])
    );

    for (const perm of requiredPermissions) {
      manifest['uses-permission'].push({ $: { 'android:name': perm } });
    }

    for (const perm of removePermissions) {
      manifest['uses-permission'].push({
        $: { 'android:name': perm, 'tools:node': 'remove' },
      });
    }

    // Set android:allowBackup="false" on the <application> element — a wallet's
    // local state (accounts, endpoints, dApp sessions) should not be copied into
    // the user's Google cloud backup / device-transfer.
    const app = manifest.application?.[0];
    if (app) {
      app.$['android:allowBackup'] = 'false';
    }

    return mod;
  });
}

// ---------------------------------------------------------------------------
// Android – Copy Kotlin source files and register ReactPackages
// ---------------------------------------------------------------------------

function withAndroidSourceFiles(config) {
  return withDangerousMod(config, [
    'android',
    (mod) => {
      const projectRoot = mod.modRequest.projectRoot;
      const androidJavaDir = path.join(
        projectRoot,
        'android',
        'app',
        'src',
        'main',
        'java',
        'com',
        'velawallet'
      );

      // Copy Kotlin files for each module (vela-cloud-sync omitted — no JS consumer)
      const moduleMappings = [
        { name: 'vela-passkey', subdir: 'passkey' },
        { name: 'vela-wallet-webview', subdir: 'webview' },
      ];

      for (const { name, subdir } of moduleMappings) {
        const srcDir = path.join(
          projectRoot,
          'modules',
          name,
          'android',
          'src',
          'main',
          'java',
          'com',
          'velawallet',
          subdir
        );
        const destDir = path.join(androidJavaDir, subdir);
        copyFilesRecursive(srcDir, destDir);
      }

      // Register ReactPackages in MainApplication
      registerAndroidPackages(projectRoot);

      return mod;
    },
  ]);
}

function registerAndroidPackages(projectRoot) {
  const mainAppDir = path.join(
    projectRoot,
    'android',
    'app',
    'src',
    'main',
    'java',
    'com',
    'velawallet',
    'wallet'
  );

  // Try both .kt and .java
  const ktPath = path.join(mainAppDir, 'MainApplication.kt');
  const javaPath = path.join(mainAppDir, 'MainApplication.java');

  // Also check the root package directory
  const altDir = path.join(
    projectRoot,
    'android',
    'app',
    'src',
    'main',
    'java',
    'app',
    'getvela',
    'wallet'
  );
  const altKtPath = path.join(altDir, 'MainApplication.kt');
  const altJavaPath = path.join(altDir, 'MainApplication.java');

  let mainAppPath = null;
  for (const candidate of [ktPath, javaPath, altKtPath, altJavaPath]) {
    if (fs.existsSync(candidate)) {
      mainAppPath = candidate;
      break;
    }
  }

  if (!mainAppPath) {
    console.warn(
      '[with-native-modules] MainApplication not found – skipping package registration. ' +
      'You may need to register the packages manually.'
    );
    return;
  }

  let content = fs.readFileSync(mainAppPath, 'utf8');

  // vela-cloud-sync omitted — no JS consumer yet.
  const imports = [
    'import com.velawallet.passkey.VelaPasskeyPackage',
    'import com.velawallet.webview.WalletWebViewPackage',
  ];

  const packageRegistrations = [
    'add(VelaPasskeyPackage())',
    'add(WalletWebViewPackage())',
  ];

  // Add imports (after the last existing import line)
  for (const imp of imports) {
    if (!content.includes(imp)) {
      // Insert after the last import statement
      const lastImportIdx = content.lastIndexOf('\nimport ');
      if (lastImportIdx !== -1) {
        const endOfLine = content.indexOf('\n', lastImportIdx + 1);
        content =
          content.slice(0, endOfLine + 1) +
          imp +
          '\n' +
          content.slice(endOfLine + 1);
      }
    }
  }

  // Add package registrations inside getPackages()
  for (const reg of packageRegistrations) {
    if (!content.includes(reg)) {
      // Look for the getPackages method and add after "val packages = ..." or
      // after "PackageList(this).packages" pattern
      const packagesPattern = /PackageList\(this\)\.packages/;
      const match = content.match(packagesPattern);
      if (match) {
        const insertIdx = content.indexOf('\n', match.index) + 1;
        content =
          content.slice(0, insertIdx) +
          '            ' +
          reg +
          '\n' +
          content.slice(insertIdx);
      }
    }
  }

  fs.writeFileSync(mainAppPath, content, 'utf8');
}

// ---------------------------------------------------------------------------
// iOS – Add native module files to Xcode project (pbxproj)
// ---------------------------------------------------------------------------

function withXcodeProjectFiles(config) {
  return withXcodeProject(config, (mod) => {
    const project = mod.modResults;
    const projectName = mod.modRequest.projectName || 'velawallet';

    // Find the main group for the app target
    const mainGroup = project.getFirstProject().firstProject.mainGroup;
    const appGroupKey = Object.keys(project.hash.project.objects.PBXGroup)
      .find((key) => {
        const group = project.hash.project.objects.PBXGroup[key];
        return typeof group === 'object' && group.name === projectName;
      });

    // Native module files to add (vela-cloud-sync omitted — no JS consumer yet)
    const nativeFiles = [
      'VelaPasskeyModule.swift',
      'VelaPasskeyModule.m',
      'VelaAppGroupModule.swift',
      'VelaAppGroupModule.m',
      'WalletWebView.swift',
      'WalletWebViewManager.swift',
      'WalletWebViewManager.m',
    ];

    for (const fileName of nativeFiles) {
      // Skip if already in project
      const alreadyAdded = Object.values(project.hash.project.objects.PBXFileReference || {})
        .some((ref) => typeof ref === 'object' && (ref.name === fileName || ref.path === fileName));

      if (alreadyAdded) continue;

      // Path must be relative to the group's sourceTree.
      // For the app group, files are at <projectName>/<file> relative to ios/
      const filePath = `${projectName}/${fileName}`;
      project.addSourceFile(filePath, { target: project.getFirstTarget().uuid }, appGroupKey);
    }

    return mod;
  });
}

// ---------------------------------------------------------------------------
// Android – Add gradle dependencies for native modules
// ---------------------------------------------------------------------------

function withAndroidDependencies(config) {
  return withDangerousMod(config, [
    'android',
    (mod) => {
      const buildGradlePath = path.join(
        mod.modRequest.projectRoot, 'android', 'app', 'build.gradle',
      );
      if (!fs.existsSync(buildGradlePath)) return mod;

      let content = fs.readFileSync(buildGradlePath, 'utf8');

      const deps = [
        'implementation("androidx.credentials:credentials:1.5.0")',
        'implementation("androidx.credentials:credentials-play-services-auth:1.5.0")',
        'implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")',
        // In-app dApp browser WebView: document-start injection + WebMessageListener.
        'implementation("androidx.webkit:webkit:1.11.0")',
      ];

      for (const dep of deps) {
        if (!content.includes(dep)) {
          // Insert after the first implementation line
          const insertPoint = content.indexOf('implementation("com.facebook.react:react-android")');
          if (insertPoint !== -1) {
            const endOfLine = content.indexOf('\n', insertPoint);
            content = content.slice(0, endOfLine + 1) +
              '    ' + dep + '\n' +
              content.slice(endOfLine + 1);
          }
        }
      }

      // Use global debug keystore so SHA256 matches assetlinks.json
      content = content.replace(
        "storeFile file('debug.keystore')",
        "storeFile file(System.getProperty('user.home') + '/.android/debug.keystore')",
      );

      fs.writeFileSync(buildGradlePath, content, 'utf8');
      return mod;
    },
  ]);
}

// ---------------------------------------------------------------------------
// Main plugin – composes all sub-plugins
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// iOS – Inject Mac IP into ip.txt for physical device Metro discovery
// ---------------------------------------------------------------------------

function withMetroHostInjection(config) {
  return withXcodeProject(config, (mod) => {
    const project = mod.modResults;
    const target = project.getFirstTarget().uuid;
    const shellScript =
      'if [ "$CONFIGURATION" = "Debug" ]; then\n' +
      '  IP=$(ipconfig getifaddr en0 || echo "localhost")\n' +
      '  echo "$IP" > "${CONFIGURATION_BUILD_DIR}/${UNLOCALIZED_RESOURCES_FOLDER_PATH}/ip.txt"\n' +
      'fi\n';

    // Avoid adding duplicate
    const buildPhases = project.hash.project.objects.PBXShellScriptBuildPhase || {};
    const alreadyExists = Object.values(buildPhases).some(
      (phase) => typeof phase === 'object' && phase.name === '"Inject Metro Host IP"'
    );
    if (!alreadyExists) {
      project.addBuildPhase([], 'PBXShellScriptBuildPhase', 'Inject Metro Host IP', target, {
        shellPath: '/bin/sh',
        shellScript,
      });
    }

    return mod;
  });
}

function withNativeModules(config) {
  // iOS
  config = withIOSInfoPlist(config);
  config = withIOSEntitlements(config);
  config = withIOSSourceFiles(config);
  config = withXcodeProjectFiles(config);
  config = withMetroHostInjection(config);

  // Android
  config = withAndroidPermissions(config);
  config = withAndroidSourceFiles(config);
  config = withAndroidDependencies(config);

  return config;
}

module.exports = withNativeModules;
