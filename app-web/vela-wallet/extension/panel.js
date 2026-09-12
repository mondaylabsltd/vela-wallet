/**
 * The side panel's doorway: pick the locale, go to the request page.
 *
 * Bundled by build.mjs like the other page-side scripts. Runs in an extension
 * page, so `chrome.*` is available; `chrome.i18n.getUILanguage()` is the same
 * fact the service worker negotiates the wallet tab from.
 */
/* global chrome */
import { negotiate, requestPage } from './lib/locales.js';

const locale = negotiate(chrome.i18n?.getUILanguage?.());
location.replace(chrome.runtime.getURL(requestPage(locale)));
