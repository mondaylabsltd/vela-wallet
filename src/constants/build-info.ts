// Build-time metadata, injected by app.config.js at build/export time and read
// from the embedded Expo config — no codegen, nothing to regenerate or commit.
import Constants from 'expo-constants';

export const APP_VERSION: string = Constants.expoConfig?.version ?? '0.0.0';
export const GIT_COMMIT: string = Constants.expoConfig?.extra?.gitCommit ?? 'unknown';
