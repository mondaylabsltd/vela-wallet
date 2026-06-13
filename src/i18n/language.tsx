/**
 * LanguageProvider — Context wrapper that drives instant, restart-free language
 * switching. Mirrors ColorSchemeProvider (constants/color-scheme.ts).
 *
 * On change, react-i18next re-renders every component using `useTranslation()`;
 * additionally the `resolved` value flips, which _layout.tsx folds into the
 * Stack `key` to remount the tree — a belt-and-suspenders refresh for any text
 * read outside the hook.
 */
import React, { createContext, useCallback, useContext, useMemo, useState } from 'react';
import {
  detectSystemLanguage,
  getLanguagePreference,
  resolveLanguage,
  setLanguagePreference,
  type AppLanguage,
  type LanguagePreference,
} from './index';

interface LanguageContextValue {
  /** What the user picked: 'auto' | 'en' | 'zh'. */
  preference: LanguagePreference;
  /** The concrete language currently rendered. */
  resolved: AppLanguage;
  /** What 'auto' resolves to right now (the device language). */
  systemLanguage: AppLanguage;
  setPreference: (pref: LanguagePreference) => void;
}

const LanguageContext = createContext<LanguageContextValue>({
  preference: 'auto',
  resolved: 'en',
  systemLanguage: 'en',
  setPreference: () => {},
});

export function useLanguagePreference() {
  return useContext(LanguageContext);
}

export function LanguageProvider({ children }: { children: React.ReactNode }) {
  const [preference, setPreferenceState] = useState<LanguagePreference>(getLanguagePreference());
  const systemLanguage = detectSystemLanguage();
  const resolved = resolveLanguage(preference);

  const setPreference = useCallback((pref: LanguagePreference) => {
    // Update module cache + i18next + persist (fires the react-i18next re-render).
    setLanguagePreference(pref);
    // Re-render this provider → `resolved` changes → Stack key remounts the tree.
    setPreferenceState(pref);
  }, []);

  const value = useMemo(
    () => ({ preference, resolved, systemLanguage, setPreference }),
    [preference, resolved, systemLanguage, setPreference],
  );

  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}
