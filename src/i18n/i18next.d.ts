/**
 * Typed translation keys — `t('settings.title')` is now autocompleted and
 * type-checked against en.json. Adding a key to en.json makes it available;
 * a typo in a t() call becomes a compile error.
 */
import 'i18next';
import type { en } from './resources';

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: {
      translation: typeof en;
    };
  }
}
