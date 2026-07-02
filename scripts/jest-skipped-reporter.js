const path = require('path');

class SkippedTestsReporter {
  onRunComplete(_testContexts, results) {
    const suites = results.testResults
      .map((suite) => ({
        file: path.relative(process.cwd(), suite.testFilePath),
        skipped: suite.skipped,
        tests: suite.testResults.filter((test) => test.status === 'pending'),
      }))
      .filter((suite) => suite.tests.length > 0);

    if (suites.length === 0) return;

    const skippedTestCount = suites.reduce((count, suite) => count + suite.tests.length, 0);
    const skippedSuiteCount = suites.filter((suite) => suite.skipped).length;

    console.log('\nSkipped tests:');
    for (const suite of suites) {
      const suffix = suite.skipped ? ' (entire test suite skipped)' : '';
      console.log(`  ${suite.file}${suffix}`);

      for (const test of suite.tests) {
        const fullName = [...test.ancestorTitles, test.title].join(' › ');
        console.log(`    ○ ${fullName}`);
      }
    }

    console.log(
      `\nSkipped: ${skippedTestCount} test${skippedTestCount === 1 ? '' : 's'} in ${suites.length} file${suites.length === 1 ? '' : 's'}`
      + (skippedSuiteCount > 0
        ? ` (${skippedSuiteCount} entire test suite${skippedSuiteCount === 1 ? '' : 's'})`
        : ''),
    );
  }
}

module.exports = SkippedTestsReporter;
