/**
 * Color scheme preference system — auto / light / dark.
 *
 * Follows the same pattern as text-scale.ts:
 *   - Module-level cache for synchronous startup
 *   - AsyncStorage persistence
 *   - React context for reactive updates
 */
import { Appearance } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import React, { createContext, useContext, useState, useEffect, useCallback, useMemo } from 'react';
import { useColorScheme as useSystemColorScheme } from 'react-native';

const STORAGE_KEY = 'vela.colorScheme';

export type ColorSchemePreference = 'auto' | 'light' | 'dark';

// ---------------------------------------------------------------------------
// Synchronous module-level cache
// ---------------------------------------------------------------------------

let _preference: ColorSchemePreference = 'auto';

export function getColorSchemePreference(): ColorSchemePreference {
  return _preference;
}

/** Resolve preference + system scheme into a concrete 'light' | 'dark'. */
export function resolveColorScheme(
  pref: ColorSchemePreference,
  systemScheme: string | null | undefined,
): 'light' | 'dark' {
  if (pref === 'auto') return systemScheme === 'dark' ? 'dark' : 'light';
  return pref;
}

/** Load from storage — call once at app startup before rendering. */
export async function loadColorScheme(): Promise<void> {
  try {
    const stored = await AsyncStorage.getItem(STORAGE_KEY);
    if (stored === 'light' || stored === 'dark' || stored === 'auto') {
      _preference = stored;
    }
  } catch {
    // Use default
  }
}

// ---------------------------------------------------------------------------
// React Context
// ---------------------------------------------------------------------------

interface ColorSchemeContextValue {
  version: number;
  preference: ColorSchemePreference;
  resolved: 'light' | 'dark';
  setPreference: (pref: ColorSchemePreference) => void;
}

const ColorSchemeContext = createContext<ColorSchemeContextValue>({
  version: 0,
  preference: 'auto',
  resolved: 'light',
  setPreference: () => {},
});

export function useColorSchemePreference() {
  return useContext(ColorSchemeContext);
}

/**
 * Provider that manages color scheme state.
 * Listens to system appearance changes when preference is 'auto'.
 * Calls rebuildColors() and bumps version on every resolved change.
 */
export function ColorSchemeProvider({ children }: { children: React.ReactNode }) {
  const systemScheme = useSystemColorScheme();
  const [preference, setPreferenceState] = useState<ColorSchemePreference>(_preference);
  const [version, setVersion] = useState(0);

  const resolved = resolveColorScheme(preference, systemScheme);

  // Rebuild colors whenever the resolved scheme changes
  useEffect(() => {
    const { rebuildColors } = require('@/constants/theme');
    rebuildColors(resolved === 'dark');
    setVersion(v => v + 1);
  }, [resolved]);

  const setPreference = useCallback((pref: ColorSchemePreference) => {
    _preference = pref;
    setPreferenceState(pref);
    AsyncStorage.setItem(STORAGE_KEY, pref).catch(() => {});
  }, []);

  const value = useMemo(() => ({
    version,
    preference,
    resolved,
    setPreference,
  }), [version, preference, resolved, setPreference]);

  return React.createElement(ColorSchemeContext.Provider, { value }, children);
}
