pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "VelaWallet"
include(":app")
// Native parity Phase 4 (perf): the journeys, run on a device against the app's
// `benchmarkRelease` variant. Neither is part of assemble/test — see
// docs/android/performance.md for how to run them (and how CI will).
include(":macrobenchmark")
include(":baselineprofile")
