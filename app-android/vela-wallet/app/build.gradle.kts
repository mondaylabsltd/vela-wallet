import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
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
        // Spec 088 FR-012 — see velaVersionCode above. The name is set by hand per release.
        versionCode = velaVersionCode
        versionName = "0.9.7"

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
            optimization {
                enable = false
            }
            // The upload key, or nothing at all — never the debug key.
            signingConfig = signingConfigs.findByName("release")
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
            // Launch animations, same arrangement (spec 012).
            assets.srcDir(projectDir.resolve("build/generated/velaAnimations"))
        }
        // The parallel space's fixed keyset (spec 043, research D2): a SECOND
        // uniffi library, bindings and .so both in the debug source set, so a
        // release APK carries neither the door nor the keys. Generated by
        // rust/scripts/build-dev-fixtures.sh (bindings/kotlin-dev, gitignored;
        // src/debug/jniLibs, gitignored).
        getByName("debug") {
            kotlin.srcDir(velaRepoRoot.resolve("rust/bindings/kotlin-dev"))
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
            // The core's mark vectors MarksTest replays: a new or edited case
            // must re-run it, not leave it UP-TO-DATE.
            test.inputs.file(velaRepoRoot.resolve("rust/crates/vela-core/tests/vectors/marks.json"))
                .withPathSensitivity(PathSensitivity.NONE)
                .withPropertyName("velaMarkVectors")
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

// Spec 088 FR-001: a release build stops here, in words, when the upload key is
// not configured — before anything is compiled, so nobody uploads a bundle that
// was quietly left unsigned.
val velaCheckUploadKey = tasks.register("velaCheckUploadKey") {
    description = "Fails a release build whose Play upload key is not configured (spec 088)."
    val problems = velaUploadKeyProblems
    val unsignedAsked = velaUnsignedRelease
    doLast {
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
    dependsOn(velaCheckCoreBindings, cargoNdkBuild, syncVelaI18nAssets, syncVelaAnimationAssets)
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
    implementation(libs.androidx.webkit)
    // Spec 046: the scanner.
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.zxing.core)
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
