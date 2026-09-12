plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// Repo root (this module lives at <repo>/app-android/vela-wallet/app).
val velaRepoRoot: File = rootDir.parentFile.parentFile

android {
    namespace = "app.getvela.wallet"
    // Plain compileSdk 36: the scaffold's `release(36) { minorApiLevel = 1 }` makes
    // PackageManager fail to resolve ANY activity in the APK on real devices/emulators
    // ("Error type 3: Activity class does not exist", START_CLASS_NOT_FOUND) — verified
    // empirically 2026-08-01 on API 34 emulator; same symptom on a physical device with
    // the sibling 009 scaffold. SDK-minor targeting has no consumer in this app.
    compileSdk = 36

    defaultConfig {
        applicationId = "app.getvela.wallet"
        // 29 (Android 10) is the lowest level we can regression-test on real
        // hardware; every library floor is ≤23 and passkeys need only 28+, but
        // shipping below what we can reproduce bugs on is a support trap.
        // Keep rust/scripts/build-android.sh --platform in sync.
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Only the ABIs rust/scripts/build-android.sh produces — prunes the extra
        // legacy ABIs (mips, x86, armeabi) the JNA aar would otherwise package.
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    buildTypes {
        release {
            optimization {
                enable = false
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
            // Generated uniffi Kotlin bindings are consumed in place (spec 008 FR-009 / research D1):
            // single committed copy, regenerated only via rust/scripts/smoke-kotlin.sh.
            kotlin.srcDir(velaRepoRoot.resolve("rust/bindings/kotlin"))
            // Locale catalogs are synced from the generated assets/i18n at build time (research D3).
            // Static File (not Provider): AGP 9 disallows Providers here; the task
            // dependency is carried by the merge*Assets wiring below.
            assets.srcDir(projectDir.resolve("build/generated/velaI18n"))
            // Launch animations, same arrangement (spec 012).
            assets.srcDir(projectDir.resolve("build/generated/velaAnimations"))
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
            test.inputs.dir(velaRepoRoot.resolve("assets/i18n"))
                .withPathSensitivity(PathSensitivity.RELATIVE)
                .withPropertyName("velaI18nCatalogs")
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

val syncVelaI18nAssets = tasks.register<Sync>("syncVelaI18nAssets") {
    description = "Copies generated locale catalogs (assets/i18n) into build assets (research D3)."
    from(velaRepoRoot.resolve("assets/i18n")) {
        include("*.json")
    }
    into(layout.buildDirectory.dir("generated/velaI18n/i18n"))
}

// Launch animations (spec 012 FR-001/FR-002): docs/design/onboarding/launch is THE
// source of truth and no app keeps a copy. Only the `core` framings ship — the
// `full` pair exists to pin the apps' box ratio and is never loaded (research D0/D3).
//
// The include pattern is a GLOB, not a list, so adding a second animation needs
// no edit here (FR-004).
val syncVelaAnimationAssets = tasks.register<Sync>("syncVelaAnimationAssets") {
    description = "Copies launch animations (docs/design/onboarding/launch) into build assets (spec 012)."
    from(velaRepoRoot.resolve("docs/design/onboarding/launch")) {
        include("*-core-*.json")
    }
    into(layout.buildDirectory.dir("generated/velaAnimations/animations"))
    // A build that silently produced an animation-less app would be worse than a
    // failed one (FR-003).
    doLast {
        val produced = destinationDir.listFiles { f -> f.name.endsWith(".json") }?.size ?: 0
        check(produced >= 4) {
            "expected at least 4 launch animation assets, found $produced in $destinationDir — " +
                "is docs/design/onboarding/launch present and named " +
                "vela-wallet-launch-{phone|desktop}-core-{dark|light}.json?"
        }
    }
}

val rustHostLib = tasks.register<Exec>("rustHostLib") {
    description = "Builds the host-platform vela-core-uniffi dylib for JVM unit tests (research D14)."
    workingDir = velaRepoRoot.resolve("rust")
    commandLine("cargo", "build", "--release", "-p", "vela-core-uniffi")
    enabled = !velaSkipRustBuild
}

tasks.named("preBuild") {
    dependsOn(cargoNdkBuild, syncVelaI18nAssets, syncVelaAnimationAssets)
}
tasks.matching { it.name.startsWith("merge") && it.name.endsWith("Assets") }.configureEach {
    dependsOn(syncVelaI18nAssets, syncVelaAnimationAssets)
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
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.androidx.work.runtime)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.lottie.compose)
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
