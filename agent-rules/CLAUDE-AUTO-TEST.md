# Automated Testing Instructions

## Create Tests

```
Analyze all source code in the current project and build a comprehensive test suite:

1. Read each module, identify public APIs, state machines, external dependencies, and boundary conditions
2. Design a test checklist covering each interface: success path, failure path, boundary conditions, concurrency, resource cleanup
3. Identify dependencies that need mocking, create reusable mocks/helpers
4. Write tests module by module in order of importance (core logic > data layer > API > utilities > UI)
5. Run all tests, confirm 0 failures, report coverage

If the project already has tests, run them first to record the pass count, find untested modules, and fill gaps by importance.
```

## Run and Fix

```
Execute a build → test → fix loop, up to 10 rounds:

Each round:
1. Build the project
2. Run all tests (unit + integration + E2E)
3. All pass → done
4. Any failure → read full error → read related source code to find root cause → fix the code (do NOT delete tests to bypass) → run failed tests first to confirm → run full suite to confirm no regressions → next round

If failures remain after 10 rounds → list remaining issues.
```

## Stability Verification

```
Write a bash script to run 30 rounds of build+test cycles, launch with run_in_background:

#!/bin/bash
CYCLES=${1:-30}; PASS=0; FAIL=0; LOG="cycle-results.log"
for i in $(seq 1 $CYCLES); do
  echo "── Cycle $i/$CYCLES ──" | tee -a $LOG
  OUT=$(build_and_test_command 2>&1)
  if echo "$OUT" | grep -q "0 failed"; then
    PASS=$((PASS+1)); echo "  ✅" | tee -a $LOG
  else
    FAIL=$((FAIL+1)); echo "  ❌" | tee -a $LOG
    echo "$OUT" | grep "❌\|FAIL\|Error" >> $LOG
  fi
done
echo "FINAL: $PASS/$CYCLES passed" | tee -a $LOG
exit $FAIL

Note: Use grep "0 failed" to check success, NOT grep "passed" (which would false-match "5 failed | 185 passed").
```

## Multi-Agent Parallel

```
Launch multiple Agents in parallel (run_in_background: true), ensuring different Agents don't operate on the same file:

Agent A: Review module, output issues + severity (read-only)
Agent B: Write tests for module (only create new files)
Agent C: Run test suite, fix failures if any

After completion, the main Agent consolidates results → runs a full test suite to confirm → commit.
```

## E2E Gotchas (by project type)

The following are critical details that Claude Code cannot infer on its own and must know in advance:

**Chrome Extension:**
- Must use headless: false, must use Chrome for Testing (`npx puppeteer browsers install chrome`), regular Chrome ignores --load-extension
- Always rm -rf the user data directory before each run
- If `__name is not defined` error in page.evaluate → use string form `page.evaluate('string')` instead

**Safari Extension:**
- Does not support MV3 service workers (use background page), use browser.* not chrome.*
- First-time enable and authorization cannot be automated, requires XCUITest or manual user action

**React Native / Mobile USB Device:**
- Android: Enable USB debugging + confirm with adb devices; "unauthorized" → check phone popup + use data cable not charging cable
- iOS: Developer mode + Xcode trust + code signing
- Detox real device config: type: "android.attached" / type: "ios.device"
- Screenshots: `adb exec-out screencap -p > /tmp/s.png` / `xcrun simctl io booted screenshot`

**Swift / Xcode:**
- Run tests from CLI: `xcodebuild test -scheme X -destination '...' | xcpretty`
- Simulator: `xcrun simctl boot "iPhone 15"`

**Rust:**
- cargo test runs in parallel by default, add #[serial] for tests sharing resources

**Go:**
- Detect goroutine leaks with go.uber.org/goleak

**Server (general):**
- Use random ports (port 0) to avoid conflicts
- Database: use transaction + rollback per test

**CI (general):**
- Passes in CI but fails locally → check timezone/locale/concurrency/memory differences
- Gradle test cache false green → use --no-build-cache
