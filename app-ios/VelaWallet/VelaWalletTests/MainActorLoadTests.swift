//
//  MainActorLoadTests.swift
//  VelaWalletTests
//
//  CI's load, on a laptop. Off unless asked for.
//
//  Three CI runs in a row (2026-09-27/28) each failed a different unit test
//  that never failed locally — NetworkEventsTests, then RegistryResolverTests
//  — and every one was a test whose "done" was a clock: on the runner the whole
//  suite ran ~8× slower (a 0.1 s test took 57 s), because every `@MainActor`
//  suite in this target shares one main actor and a macos runner has three
//  cores. This holds the main actor the way that runner did, for as long as
//  asked, while the rest of the suite runs beside it:
//
//      xcodebuild test … -only-testing:VelaWalletTests \
//        TEST_RUNNER_VELA_TEST_LOAD_SECONDS=150
//
//  (`test-without-building` does not pass `TEST_RUNNER_*` through: add
//  `VELA_TEST_LOAD_SECONDS` to the target's `EnvironmentVariables` in a copy
//  of the generated `.xctestrun` and run that. Two copies of the suite on two
//  simulators beside a few `yes > /dev/null` is closest to a busy runner.)
//
//  200 tasks, each taking 30 ms turns on the main actor and yielding. A test
//  that waits on STATE passes under it, only slower; a test that waits on the
//  clock fails, which is the point.
//

import Foundation
import Testing

struct MainActorLoadTests {
    static let seconds = Double(ProcessInfo.processInfo.environment["VELA_TEST_LOAD_SECONDS"] ?? "")

    @Test(.enabled(if: seconds != nil, "set TEST_RUNNER_VELA_TEST_LOAD_SECONDS to run the suite under load"))
    func holdTheMainActorWhileTheSuiteRuns() async {
        let end = Date().addingTimeInterval(Self.seconds ?? 0)
        await withTaskGroup(of: Void.self) { group in
            for _ in 0..<200 {
                group.addTask { @MainActor in
                    while Date() < end {
                        let turn = Date().addingTimeInterval(0.03)
                        while Date() < turn {}
                        await Task.yield()
                    }
                }
            }
        }
    }
}
