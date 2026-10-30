// Native parity Phase 4: the Macrobenchmark journeys (docs/android/performance.md).
//
//   ./gradlew :macrobenchmark:connectedBenchmarkReleaseAndroidTest -PvelaSkipRustBuild
//
// Runs on a connected device or emulator against :app's `benchmarkRelease`
// (release-shaped, debug-signed, profileable), which this module installs.
plugins {
    alias(libs.plugins.android.test)
}

android {
    namespace = "app.getvela.wallet.macrobenchmark"
    compileSdk = 36

    defaultConfig {
        // Macrobenchmark needs 23+; the app's own floor is 29.
        minSdk = 29
        targetSdk = 36
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // An emulator is fine for a trend line; the numbers are only comparable
        // run to run on one device. Real hardware drops this suppression.
        testInstrumentationRunnerArguments["androidx.benchmark.suppressErrors"] = "EMULATOR"
    }

    buildTypes {
        // Matches :app's benchmarkRelease (created by the baseline-profile plugin there).
        create("benchmarkRelease") {
            isDebuggable = true
            signingConfig = signingConfigs.getByName("debug")
            matchingFallbacks += listOf("release")
        }
    }

    sourceSets {
        // The journeys, shared with :baselineprofile — one definition of "open
        // Send and reach Confirm" for the profile and for the numbers.
        getByName("main") { kotlin.srcDir("src/journeys/java") }
    }

    targetProjectPath = ":app"
    experimentalProperties["android.experimental.self-instrumenting"] = true

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

java {
    toolchain { languageVersion = JavaLanguageVersion.of(17) }
}

dependencies {
    implementation(libs.androidx.benchmark.test.ext.junit)
    implementation(libs.androidx.benchmark.test.runner)
    implementation(libs.androidx.test.uiautomator)
    implementation(libs.androidx.benchmark.macro.junit4)
}

androidComponents {
    beforeVariants(selector().all()) { variant ->
        variant.enable = variant.buildType == "benchmarkRelease"
    }
}
