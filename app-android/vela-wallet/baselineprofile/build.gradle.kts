// Native parity Phase 4: generates the baseline profile (and the startup
// profile) that :app ships in src/main/generated/baselineProfiles.
//
//   ./gradlew :app:generateBaselineProfile -PvelaSkipRustBuild
//
// Needs a connected API 33+ device or emulator (a rooted one for API < 33);
// see docs/android/performance.md.
plugins {
    alias(libs.plugins.android.test)
    alias(libs.plugins.androidx.baselineprofile)
}

android {
    namespace = "app.getvela.wallet.baselineprofile"
    compileSdk = 36

    defaultConfig {
        minSdk = 29
        targetSdk = 36
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        testInstrumentationRunnerArguments["androidx.benchmark.suppressErrors"] = "EMULATOR"
    }

    sourceSets {
        // The same journeys the macrobenchmarks time (:macrobenchmark).
        getByName("main") { kotlin.srcDir("../macrobenchmark/src/journeys/java") }
    }

    targetProjectPath = ":app"

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

java {
    toolchain { languageVersion = JavaLanguageVersion.of(17) }
}

baselineProfile {
    // The device the person running it has attached — no Gradle-managed
    // device image to download (and no disk for one here).
    useConnectedDevices = true
}

dependencies {
    implementation(libs.androidx.benchmark.test.ext.junit)
    implementation(libs.androidx.benchmark.test.runner)
    implementation(libs.androidx.test.uiautomator)
    implementation(libs.androidx.benchmark.macro.junit4)
}
