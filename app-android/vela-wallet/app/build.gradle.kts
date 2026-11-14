import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
    // Native parity Phase 4: consumes the profile :baselineprofile generates and
    // adds the release-shaped `benchmarkRelease` / `nonMinifiedRelease` build
    // types the journeys run against (docs/android/performance.md).
    alias(libs.plugins.androidx.baselineprofile)
}

// Repo root (this module lives at <repo>/app-android/vela-wallet/app).
val velaRepoRoot: File = rootDir.parentFile.parentFile

// ---------------------------------------------------------------------------
// Spec 088 FR-001: the Play upload key. Never in the repo. Each value comes from
// an environment variable, else from `app-android/vela-wallet/keystore.properties`
// (gitignored; keys `storeFile`, `storePassword`, `keyAlias`, `keyPassword`; a
// relative `storeFile` is relative to that file, `~/` is the home directory):
//
//   VELA_UPLOAD_STORE_FILE  VELA_UPLOAD_STORE_PASSWORD  VELA_UPLOAD_KEY_ALIAS  VELA_UPLOAD_KEY_PASSWORD
//
// A release build without all four FAILS, saying which are missing — it is never
// signed with the debug key and never comes out unsigned by accident. The one
// way to an unsigned release is to ask for it: `-PvelaUnsignedRelease` (CI's
// packaging check, whose artifact the founder signs by hand). Debug builds and
// unit tests read none of this.
// ---------------------------------------------------------------------------
val velaKeystoreProperties = Properties().apply {
    val file = rootDir.resolve("keystore.properties")
    if (file.isFile) file.inputStream().use { load(it) }
}

fun velaUploadValue(env: String, property: String): String? =
    providers.environmentVariable(env).orNull?.takeIf { it.isNotBlank() }
        ?: velaKeystoreProperties.getProperty(property)?.trim()?.takeIf { it.isNotEmpty() }

val velaUploadKey: Map<String, String?> = linkedMapOf(
    "VELA_UPLOAD_STORE_FILE (storeFile)" to velaUploadValue("VELA_UPLOAD_STORE_FILE", "storeFile"),
    "VELA_UPLOAD_STORE_PASSWORD (storePassword)" to velaUploadValue("VELA_UPLOAD_STORE_PASSWORD", "storePassword"),
    "VELA_UPLOAD_KEY_ALIAS (keyAlias)" to velaUploadValue("VELA_UPLOAD_KEY_ALIAS", "keyAlias"),
    "VELA_UPLOAD_KEY_PASSWORD (keyPassword)" to velaUploadValue("VELA_UPLOAD_KEY_PASSWORD", "keyPassword"),
)
val velaUploadStoreFile: File? = velaUploadKey.values.first()?.let { path ->
    when {
        path.startsWith("~/") -> File(System.getProperty("user.home"), path.removePrefix("~/"))
        File(path).isAbsolute -> File(path)
        else -> rootDir.resolve(path)
    }
}
val velaUploadKeyProblems: List<String> = buildList {
    velaUploadKey.filterValues { it == null }.keys.forEach { add("missing $it") }
    if (velaUploadStoreFile != null && !velaUploadStoreFile.isFile) add("no keystore at $velaUploadStoreFile")
}
val velaUnsignedRelease: Boolean = providers.gradleProperty("velaUnsignedRelease").isPresent
// §35, revised 2026-11-14 (owner): a test build on a phone uses getvela.app's
// passkeys, and getvela.app's assetlinks.json lists the release certificate
// only — so on a developer's own machine, where the upload key is in
// `keystore.properties`, the test builds (debug, benchmark) are signed with it
// and can create and sign in. Only from that local file: CI hands the key in
// through the environment for packaging, and its test builds stay on the debug
// key, so no debuggable APK signed with the release key leaves a CI run.
// Without the file nothing changes (debug key; passkeys refused by the system).
val velaSignTestBuildsWithUploadKey: Boolean =
    rootDir.resolve("keystore.properties").isFile &&
        velaUploadKeyProblems.isEmpty() &&
        providers.environmentVariable("VELA_UPLOAD_STORE_FILE").orNull.isNullOrBlank()
/** See the debug build type: R8 on a debug build, for a device pass of release's shrinking. */
val velaMinifyDebug: Boolean = providers.gradleProperty("velaMinifyDebug").isPresent
// A device pass on somebody's phone (`-PvelaDeviceTest`): every variant is
// built as `app.getvela.wallet.devicetest`, so it installs BESIDE the store or
// debug app already there and never over it (`adb install -r` onto a held
// wallet replaces its code and keeps its data — or, signed differently, is
// refused only after the tester has stopped looking). Its storage, keystore
// and WorkManager queue are its own. What it gives up: getvela.app's
// assetlinks.json names `app.getvela.wallet` alone, so Credential Manager
// refuses a real passkey ceremony for this package (the parallel space signs
// instead) and the App Links stay unverified (`adb shell am start -p …`
// opens them). Never an upload: `velaCheckUploadKey` refuses a signed
// release with it (an unsigned one, `-PvelaUnsignedRelease`, is for a device
// pass signed by hand with the debug key).
val velaDeviceTest: Boolean = providers.gradleProperty("velaDeviceTest").isPresent
// §35 (review 0-1): a test build's own passkey domain on a device —
// `-PvelaTestRpDomain=test.example.org`, else env `TEST_RP_DOMAIN`, else
// empty (unset). Validated by the core (`relying_party_id`), not here.
val velaTestRpDomain: String = (providers.gradleProperty("velaTestRpDomain").orNull ?: providers.environmentVariable("TEST_RP_DOMAIN").orNull)
    ?.trim().orEmpty()
    .also { require('"' !in it && '\\' !in it) { "velaTestRpDomain must be a bare host name: $it" } }

// Spec 088 FR-012: every upload needs a higher versionCode than any before it.
// `-PvelaVersionCode=N`, else `VELA_VERSION_CODE`, else the number of commits
// behind HEAD — which only grows along `main`, so a later commit is a later
// build without anybody keeping a counter. (A shallow clone counts 1: the
// packaging workflow checks out full history for that reason.)
val velaVersionCode: Int = run {
    val asked = (providers.gradleProperty("velaVersionCode").orNull ?: providers.environmentVariable("VELA_VERSION_CODE").orNull)
        ?.trim()?.takeIf { it.isNotEmpty() }
    if (asked != null) {
        asked.toIntOrNull()?.takeIf { it in 1..2_100_000_000 }
            ?: throw GradleException("velaVersionCode / VELA_VERSION_CODE must be a whole number from 1 to 2100000000, not '$asked'")
    } else {
        providers.exec {
            commandLine("git", "rev-list", "--count", "HEAD")
            isIgnoreExitValue = true
        }.standardOutput.asText.map { it.trim() }.orNull?.toIntOrNull()?.takeIf { it > 0 } ?: 1
    }
}

// The NDK that strips the packaged native libraries — the SAME one
// rust/scripts/build-android.sh cross-compiles with: ANDROID_NDK_HOME when set,
// else the newest under <sdk>/ndk. Without it AGP looks for ITS default NDK,
// which a machine (this one, CI's runner) need not have, says "Unable to strip
// the following libraries, packaging them as they are", and the arm64 core
// ships at ~40 MB with its symbol tables instead of ~16 MB (Phase 4 budget:
// ≤ 19.5 MB). Stripping here, not in the shared Cargo release profile, keeps
// the host dylib the JVM tests load (and its backtraces) as it was.
val velaNdkVersion: String? = run {
    fun revision(dir: File): String? = dir.resolve("source.properties").takeIf { it.isFile }?.readLines()
        ?.firstOrNull { it.trim().startsWith("Pkg.Revision") }
        ?.substringAfter('=')?.trim()?.takeIf { it.isNotEmpty() }
    val explicit = providers.environmentVariable("ANDROID_NDK_HOME").orNull?.takeIf { it.isNotBlank() }?.let(::File)
    if (explicit != null) return@run revision(explicit)
    val sdk = (providers.environmentVariable("ANDROID_HOME").orNull ?: providers.environmentVariable("ANDROID_SDK_ROOT").orNull)
        ?.takeIf { it.isNotBlank() }?.let(::File)
        ?: rootDir.resolve("local.properties").takeIf { it.isFile }?.let { file ->
            Properties().apply { file.inputStream().use { load(it) } }.getProperty("sdk.dir")?.let(::File)
        }
        ?: File(System.getProperty("user.home"), "Library/Android/sdk")
    sdk.resolve("ndk").listFiles { f -> f.isDirectory }
        ?.mapNotNull { dir -> revision(dir) }
        ?.maxWithOrNull(
            compareBy<String>({ it.split('.').getOrNull(0)?.toIntOrNull() ?: 0 }, { it.split('.').getOrNull(1)?.toIntOrNull() ?: 0 }, { it.split('.').getOrNull(2)?.toIntOrNull() ?: 0 }),
        )
}

android {
    namespace = "app.getvela.wallet"
    // See velaNdkVersion: the libraries are stripped with the NDK they were built with.
    velaNdkVersion?.let { ndkVersion = it }
    // Plain compileSdk 36: the scaffold's `release(36) { minorApiLevel = 1 }` makes
    // PackageManager fail to resolve ANY activity in the APK on real devices/emulators
    // ("Error type 3: Activity class does not exist", START_CLASS_NOT_FOUND) — verified
    // empirically 2026-08-01 on API 34 emulator; same symptom on a physical device with
    // the sibling 009 scaffold. SDK-minor targeting has no consumer in this app.
    compileSdk = 36

    defaultConfig {
        applicationId = "app.getvela.wallet"
        if (velaDeviceTest) applicationIdSuffix = ".devicetest"
        // Real-device pass 2 (D10): the device-test build is labelled "Vela
        // Test" on the launcher, the widget and the tile — never "Vela
        // Wallet" beside the owner's own app (iOS's device-test build is
        // named the same). The store and debug builds keep `app_name`.
        manifestPlaceholders["velaAppLabel"] = if (velaDeviceTest) "Vela Test" else "@string/app_name"
        // 29 (Android 10) is the lowest level we can regression-test on real
        // hardware; every library floor is ≤23 and passkeys need only 28+, but
        // shipping below what we can reproduce bugs on is a support trap.
        // Keep rust/scripts/build-android.sh --platform in sync.
        minSdk = 29
        targetSdk = 36
        // Spec 088 FR-012 — see velaVersionCode above. The name is set by hand per release.
        versionCode = velaVersionCode
        versionName = "0.9.8"

        // Spec 047: the About page and the bug report name the build. A provider,
        // not a process at configuration time — the configuration cache refuses that.
        // Spec 064 §3, the same order every shell follows: `VELA_GIT_COMMIT` when the
        // build sets it (CI does, from the commit it checked out — a container often
        // cannot ask git itself), else git, else the word `unknown`. Seven characters,
        // like every other shell, so one commit reads the same on all of them.
        val gitCommit = providers.environmentVariable("VELA_GIT_COMMIT")
            .map { it.trim() }
            .filter { it.isNotEmpty() }
            .orElse(
                providers.exec {
                    commandLine("git", "rev-parse", "HEAD")
                    isIgnoreExitValue = true
                }.standardOutput.asText.map { it.trim() },
            )
            .map { if (it.length >= 7) it.take(7) else "unknown" }
            .getOrElse("unknown")
        buildConfigField("String", "GIT_COMMIT", "\"$gitCommit\"")
        // Spec 106: clientmcp.dev, unless a development build names another
        // (`-PvelaClientmcpSite=https://staging.clientmcp.dev`).
        val clientmcpSite = providers.gradleProperty("velaClientmcpSite")
            .orElse("https://clientmcp.dev")
            .get()
        buildConfigField("String", "CLIENTMCP_SITE", "\"$clientmcpSite\"")
        // §28: a test build never writes to production. The device-test
        // flavour is a test build whatever its build type (an unsigned
        // release for a device pass included); debug builds are too
        // (`BuildConfig.DEBUG`). `-PvelaAllowProductionWrites` is the opt-in,
        // off by default and meaningless in a store build
        // (`ProductionWriteGuard`).
        buildConfigField("boolean", "DEVICE_TEST", "$velaDeviceTest")
        buildConfigField(
            "boolean",
            "ALLOW_PRODUCTION_WRITES",
            "${providers.gradleProperty("velaAllowProductionWrites").isPresent}",
        )
        // §35 (review 0-1): the passkey domain a TEST build on a device uses
        // (`-PvelaTestRpDomain=…`, else env `TEST_RP_DOMAIN`; empty = unset).
        // The rule is the core's (`relying_party_id`, `RelyingParty`): a
        // release build uses getvela.app and never reads it, an emulator uses
        // localhost, and a device test build without one refuses its ceremony
        // in the core's words. A store release carries it empty (below).
        buildConfigField("String", "TEST_RP_DOMAIN", "\"$velaTestRpDomain\"")

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Only the ABIs rust/scripts/build-android.sh produces — prunes the extra
        // legacy ABIs (mips, x86, armeabi) the JNA aar would otherwise package.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    signingConfigs {
        // Only when all four values are there; the check below says what is not.
        if (velaUploadKeyProblems.isEmpty()) {
            create("release") {
                storeFile = velaUploadStoreFile
                storePassword = velaUploadKey.values.elementAt(1)
                keyAlias = velaUploadKey.values.elementAt(2)
                keyPassword = velaUploadKey.values.elementAt(3)
            }
        }
    }

    buildTypes {
        release {
            // R8: shrunk, optimized, renamed (Phase 4 follow-up, nat-android-6).
            // The keep rules are src/main/keepRules (every rule says why);
            // `scripts/check-release-apk.sh` and a device pass on the release
            // build are the check that nothing reflective was renamed away.
            // The mapping file (build/outputs/mapping/release/mapping.txt) is
            // what retraces a crash report from this build: keep it with it.
            optimization {
                enable = true
            }
            // The upload key, or nothing at all — never the debug key.
            signingConfig = signingConfigs.findByName("release")
            // §35: a store release never carries a test passkey domain (the
            // core never reads it for a release build either); a device-test
            // release is a test build and keeps the one it was given.
            // `scripts/check-release-apk.sh` holds this.
            if (!velaDeviceTest) buildConfigField("String", "TEST_RP_DOMAIN", "\"\"")
        }
        // See velaSignTestBuildsWithUploadKey: the upload key from this
        // machine's keystore.properties, so a phone lets it use getvela.app.
        if (velaSignTestBuildsWithUploadKey) {
            debug {
                signingConfig = signingConfigs.getByName("release")
            }
        }
        // A DEBUG build with release's R8 (`-PvelaMinifyDebug`): the doors only
        // debug has — the parallel space's fixed keys, the harness's launch
        // inputs — on code shrunk and renamed by the same rules, so a device
        // pass can walk create, send and sign on what R8 makes of them.
        if (velaMinifyDebug) {
            debug {
                optimization {
                    enable = true
                }
            }
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        compose = true
        // `BuildConfig.DEBUG` is the whole gate on the on-device diagnostic log
        // (spec 019): a release build carries the calls and does nothing.
        buildConfig = true
    }

    sourceSets {
        getByName("main") {
            // Generated uniffi Kotlin bindings are consumed in place (spec 008 FR-009 / research D1).
            // Gitignored: each checkout generates them with rust/scripts/build-kotlin-bindings.sh,
            // which stamps the core they came from — `velaCheckCoreBindings` below refuses a stale one.
            kotlin.srcDir(velaRepoRoot.resolve("rust/bindings/kotlin"))
            // Locale catalogs are synced from the generated assets/i18n at build time (research D3).
            // Static File (not Provider): AGP 9 disallows Providers here; the task
            // dependency is carried by the merge*Assets wiring below.
            assets.srcDir(projectDir.resolve("build/generated/velaI18n"))
        }
        // The parallel space's fixed keyset (spec 043, research D2): a SECOND
        // uniffi library, bindings and .so both in the debug source set, so a
        // release APK carries neither the door nor the keys. Generated by
        // rust/scripts/build-dev-fixtures.sh (bindings/kotlin-dev, gitignored;
        // src/debug/jniLibs, gitignored).
        getByName("debug") {
            kotlin.srcDir(velaRepoRoot.resolve("rust/bindings/kotlin-dev"))
        }
        // The multisig's simulated world (MultisigWorld.kt), shared by the JVM
        // flow tests (testDebug: the host dylib) and the instrumented ones (the
        // app's own .so) — both sign with the dev-fixtures keyset, debug only.
        // On a device it reads Safe's canonical code from the test APK's assets.
        getByName("testDebug") {
            kotlin.srcDir("src/sharedTest/java")
        }
        getByName("androidTest") {
            kotlin.srcDir("src/sharedTest/java")
            assets.srcDir(velaRepoRoot.resolve("rust/crates/vela-core/tests/vectors/safe-code"))
        }
        // Phase 4: the baseline-profile plugin's build types are release with
        // another name — so they take release's own sources (the no-op
        // parallel-space and launch-input bindings), plus the one thing a
        // journey needs that release must never carry: a door to put a fixture
        // wallet in the store (src/benchmark, see BenchSeedActivity; its
        // manifest is added per variant below — the plugin's build types do
        // not pick up a src/<type>/AndroidManifest.xml).
        // check-release-apk.sh fails a release package that carries it.
        for (benchmarkType in listOf("benchmarkRelease", "nonMinifiedRelease")) {
            maybeCreate(benchmarkType).apply {
                kotlin.srcDir("src/release/java")
                kotlin.srcDir("src/benchmark/java")
            }
        }
    }

    testOptions {
        unitTests.all { test ->
            // JVM engine tests load the host-platform dylib through JNA (research D14).
            test.systemProperty("jna.library.path", velaRepoRoot.resolve("rust/target/release").absolutePath)
            test.systemProperty("vela.repo.root", velaRepoRoot.absolutePath)
            // The system properties above are just path STRINGS to Gradle — declare the
            // files behind them as tracked inputs, or the drift/engine tests go
            // stale-green (UP-TO-DATE) exactly when the guarded files change.
            test.inputs.file(velaRepoRoot.resolve("docs/design-tokens.json"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaDesignTokens")
            // The web's tokens: the value button's fill (a web addition) is pinned to them.
            test.inputs.file(velaRepoRoot.resolve("app-web/vela-wallet/src/lib/tokens/tokens.css"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaWebTokens")
            // The App Links the owner hosts (AppLinksTest reads both).
            test.inputs.file(projectDir.resolve("src/main/AndroidManifest.xml"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaManifest")
            test.inputs.file(rootDir.resolve("app-links/assetlinks.json"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaAssetLinks")
            test.inputs.dir(velaRepoRoot.resolve("assets/i18n"))
                .withPathSensitivity(PathSensitivity.RELATIVE)
                .withPropertyName("velaI18nCatalogs")
            // The core's mark vectors MarksTest replays: a new or edited case
            // must re-run it, not leave it UP-TO-DATE.
            test.inputs.file(velaRepoRoot.resolve("rust/crates/vela-core/tests/vectors/marks.json"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaMarkVectors")
            // The launch animation's replay vectors LaunchGateTest replays.
            test.inputs.file(velaRepoRoot.resolve("rust/crates/vela-core/tests/vectors/launch/launch-animation.json"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaLaunchVectors")
            // The launch timeline's samples LaunchTimelineTest replays.
            test.inputs.file(velaRepoRoot.resolve("docs/design/onboarding/launch/launch-timeline.vectors.json"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaLaunchTimelineVectors")
            // The ts-rs mirrors CoreWireDriftTest checks the Kotlin wire types
            // against (spec 040 FR-008). Same stale-green hazard as the tokens
            // above, and worse: without this, a Rust rename lands, the mirrors
            // regenerate, and the one test that would have caught it is skipped
            // as UP-TO-DATE.
            test.inputs.dir(
                velaRepoRoot.resolve("app-web/vela-wallet/src/lib/core/generated"),
            )
                .withPathSensitivity(PathSensitivity.RELATIVE)
                .withPropertyName("velaCoreWireMirrors")
            test.inputs.file(
                velaRepoRoot.resolve("rust/target/release/${System.mapLibraryName("vela_core_uniffi")}"),
            )
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaHostEngineLib")
        }
    }
}

// Native parity Phase 4. The profile is generated on a device by
// `:app:generateBaselineProfile` (never during an ordinary build — it needs an
// emulator) and checked in under src/main/generated/baselineProfiles, so every
// variant, release included, ships it; Play installs it from the bundle's dex
// metadata and androidx.profileinstaller does for a sideloaded APK.
baselineProfile {
    automaticGenerationDuringBuild = false
    saveInSrc = true
    mergeIntoMain = true
    // The startup profile (src/main/generated/baselineProfiles/startup-prof.txt,
    // the launch journey's classes) lays out the primary dex so a cold start
    // reads the classes it needs from as few pages as possible. AGP applies it
    // only to an R8-optimized build — which release now is.
    dexLayoutOptimization = true
}

// The benchmark build types are signed with the debug key so a device will
// install them — or, on a machine whose keystore.properties holds the upload
// key, with that key (velaSignTestBuildsWithUploadKey). They are never
// uploaded (the release check above stays on `preReleaseBuild` alone).
android.buildTypes.matching { it.name == "benchmarkRelease" || it.name == "nonMinifiedRelease" }.configureEach {
    signingConfig = android.signingConfigs.getByName(if (velaSignTestBuildsWithUploadKey) "release" else "debug")
}
// The plugin turns minification off for `nonMinifiedRelease` through the old
// DSL; release's `optimization { enable = true }` (AGP 9) still reached it, so
// the generator collected rules against renamed classes (and BenchSeedActivity
// crashed under R8). The profile is collected on the original names; R8
// rewrites them for the optimized builds.
// Set in finalizeDsl, which runs after the plugin's own (it copies release
// into the build types it creates there).
androidComponents.finalizeDsl { extension ->
    extension.buildTypes.findByName("nonMinifiedRelease")?.optimization?.enable = false
}
androidComponents {
    onVariants(selector().all()) { variant ->
        if (variant.buildType == "benchmarkRelease" || variant.buildType == "nonMinifiedRelease") {
            variant.sources.manifests.addStaticManifestFile("src/benchmark/AndroidManifest.xml")
        }
    }
}

// Evaluated at configuration time (configuration-cache safe).
val velaSkipRustBuild: Boolean = providers.gradleProperty("velaSkipRustBuild").isPresent

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    description = "Cross-compiles libvela_core_uniffi.so for all packaged ABIs (research D2)."
    workingDir = velaRepoRoot
    commandLine("bash", velaRepoRoot.resolve("rust/scripts/build-android.sh").absolutePath)
    enabled = !velaSkipRustBuild

    // WITHOUT these, an Exec task declares no outputs and therefore can NEVER be
    // UP-TO-DATE: Gradle re-runs the full three-ABI Rust release cross-compile on
    // every single build. Measured at ~6 minutes on an M-series Mac. Command-line
    // builds hid it because they pass -PvelaSkipRustBuild; Android Studio does
    // not, so the IDE paid it every time.
    //
    // cargo's own incremental check is fast but it never gets to run — Gradle
    // spawns the process first. Declaring the real inputs and outputs lets
    // Gradle skip the spawn entirely when nothing in the Rust tree moved.
    inputs.files(
        velaRepoRoot.resolve("rust/Cargo.toml"),
        velaRepoRoot.resolve("rust/Cargo.lock"),
    ).withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.dir(velaRepoRoot.resolve("rust/crates"))
        .withPathSensitivity(PathSensitivity.RELATIVE)
        .withPropertyName("velaRustSources")
    inputs.file(velaRepoRoot.resolve("rust/scripts/build-android.sh"))
        .withPathSensitivity(PathSensitivity.RELATIVE)
    // The script writes here (see rust/scripts/build-android.sh).
    outputs.dir(projectDir.resolve("src/main/jniLibs"))
        .withPropertyName("velaJniLibs")
    outputs.cacheIf { true }
}

// Spec 088 FR-001: a release build stops here, in words, when the upload key is
// not configured — before anything is compiled, so nobody uploads a bundle that
// was quietly left unsigned.
val velaCheckUploadKey = tasks.register("velaCheckUploadKey") {
    description = "Fails a release build whose Play upload key is not configured (spec 088)."
    val problems = velaUploadKeyProblems
    val unsignedAsked = velaUnsignedRelease
    val deviceTest = velaDeviceTest
    doLast {
        if (deviceTest && !unsignedAsked) {
            throw GradleException(
                "-PvelaDeviceTest builds app.getvela.wallet.devicetest, which is never uploaded. " +
                    "For a device pass of the release build, add -PvelaUnsignedRelease and sign the APK with the debug key.",
            )
        }
        if (problems.isNotEmpty() && !unsignedAsked) {
            throw GradleException(
                "The release build has no upload key, so it would not be accepted by Google Play.\n" +
                    problems.joinToString("\n") { "  - $it" } + "\n" +
                    "Set the four VELA_UPLOAD_* environment variables, or write them to " +
                    "app-android/vela-wallet/keystore.properties (gitignored) as storeFile, storePassword, " +
                    "keyAlias and keyPassword. See docs/NATIVE-LAUNCH-CHECKLIST.md §A1. " +
                    "To build an UNSIGNED release on purpose (CI's packaging check), pass -PvelaUnsignedRelease.",
            )
        }
    }
}
tasks.matching { it.name == "preReleaseBuild" }.configureEach {
    dependsOn(velaCheckUploadKey)
}

val syncVelaI18nAssets = tasks.register<Sync>("syncVelaI18nAssets") {
    description = "Copies generated locale catalogs (assets/i18n) into build assets (research D3)."
    from(velaRepoRoot.resolve("assets/i18n")) {
        include("*.json")
        // P2.6: the on-demand namespaces (gen-i18n's ON_DEMAND, e.g.
        // multisig/<lng>.json = home.multisig), packaged beside the main
        // catalogs and deep-merged in by I18nRuntime.include when a screen
        // that draws them opens. A glob, so a new namespace needs no edit here.
        include("*/*.json")
    }
    into(layout.buildDirectory.dir("generated/velaI18n/i18n"))
    // An app that silently shipped without a namespace would show raw keys on
    // the screens that need it — fail the build instead.
    doLast {
        val missing = listOf("multisig").filter { name ->
            (destinationDir.resolve(name).listFiles { f -> f.name.endsWith(".json") }?.size ?: 0) == 0
        }
        check(missing.isEmpty()) {
            "on-demand i18n namespaces missing from $destinationDir: $missing — is assets/i18n/<name>/ generated (npm --prefix scripts run gen:i18n)?"
        }
    }
}

// AGP compiles against a jlink'd image of the platform's system modules, and
// it runs `jlink` from the JDK GRADLE ITSELF is running on — not from
// `JAVA_HOME`, which Gradle only consults when it starts a new daemon. A daemon
// started by an IDE whose bundled Java is a JRE (VS Code's Java extension ships
// one, without jlink) therefore fails every build with
// "jlink executable ... does not exist", and reusing that daemon makes the
// failure survive `JAVA_HOME=... ./gradlew`, which is what made it look
// intermittent. A toolchain settles it: the image is built with THIS JDK
// whatever launched Gradle, and the foojay resolver in settings.gradle.kts
// fetches one if the machine has none.
java {
    toolchain { languageVersion = JavaLanguageVersion.of(17) }
}

val rustHostLib = tasks.register<Exec>("rustHostLib") {
    description = "Builds the host-platform vela-core-uniffi dylib for JVM unit tests (research D14)."
    workingDir = velaRepoRoot.resolve("rust")
    commandLine("cargo", "build", "--release", "-p", "vela-core-uniffi")
    enabled = !velaSkipRustBuild
}

// The Kotlin bindings in rust/bindings/kotlin are gitignored, generated once,
// and then go on describing whatever core was in the tree that day. Gradle
// cross-compiles the three .so files itself but has never regenerated the
// Kotlin beside them, so a core change lands as forty `Unresolved reference`
// errors about functions that plainly exist — a compiler error that reads like
// a code bug and is not one. It cost 0.9.5 and 0.9.6 a failed build each.
//
// A check, not a regeneration: in a worktree several sessions share, rebuilding
// to silence this would bake somebody else's unfinished work into the APK. The
// script says which case it is and what to run.
val velaCheckCoreBindings = tasks.register<Exec>("velaCheckCoreBindings") {
    description = "Fails before Kotlin compiles if rust/bindings/kotlin is not this tree's core."
    workingDir = velaRepoRoot
    commandLine("bash", velaRepoRoot.resolve("rust/scripts/check-android-core-fresh.sh").absolutePath)
    // It compares a stamp with a hash of the Rust tree; nothing it reads makes
    // it skippable, and it costs about a second.
    outputs.upToDateWhen { false }
}

tasks.named("preBuild") {
    dependsOn(velaCheckCoreBindings, cargoNdkBuild, syncVelaI18nAssets)
}
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }.configureEach {
    dependsOn(syncVelaI18nAssets)
}
tasks.withType<Test>().configureEach {
    dependsOn(rustHostLib)
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.credentials)
    implementation(libs.androidx.credentials.play.services.auth)
    implementation(libs.play.services.fido)
    // The caBLE WebSocket tunnel — the CTAP 2.2 channel of "sign in with your
    // phone". The BLE-only CTAP 2.3 channel needs no HTTP library at all.
    implementation(libs.okhttp)
    implementation(libs.kotlinx.coroutines.play.services)
    implementation(libs.androidx.core.splashscreen)
    // Phase 4: installs the shipped baseline profile where Play does not (sideloads, benchmarks).
    implementation(libs.androidx.profileinstaller)
    baselineProfile(project(":baselineprofile"))
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.work.runtime)
    // Native parity Phase 3: the home-screen balance widget (no network on it).
    implementation(libs.androidx.glance.appwidget)
    implementation(libs.androidx.webkit)
    // Spec 046: the scanner.
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.zxing.core)
    implementation(libs.kotlinx.serialization.json)
    // Used directly (StateFlow, launch) — do not rely on lifecycle's transitive edge.
    implementation(libs.kotlinx.coroutines.android)
    // JNA: Android needs the aar (bundled libjnidispatch.so per ABI); JVM tests use the plain jar.
    implementation(libs.jna) {
        artifact {
            type = "aar"
        }
    }
    testImplementation(libs.junit)
    testImplementation(libs.jna)
    testImplementation(libs.org.json)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.junit)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    debugImplementation(libs.androidx.compose.ui.tooling)
}
