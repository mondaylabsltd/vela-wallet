// Dynamic Expo config: extends app.json and injects build-time metadata.
// This runs at build/export time (expo start / expo export / expo run:*), so the
// git commit lands in the bundle via expo-constants — no generated source file,
// the repo stays clean. See src/constants/build-info.ts for the consumer side.
const { execSync } = require('child_process');

function resolveGitCommit() {
  // CI/CD builds may not have a full .git (or any) — prefer platform-provided SHAs.
  const fromEnv = process.env.CF_PAGES_COMMIT_SHA || process.env.GITHUB_SHA;
  if (fromEnv) return fromEnv.slice(0, 7);
  try {
    return execSync('git rev-parse --short HEAD', { encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

module.exports = ({ config }) => ({
  ...config,
  extra: {
    ...config.extra,
    gitCommit: resolveGitCommit(),
  },
});
