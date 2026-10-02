/**
 * The doorway of the request window and the wallet tab (spec 086, issue 317).
 *
 * The worker used to open both in Chrome's UI language: a service worker has
 * no `localStorage`, so it could not read the language the person pinned in
 * Settings, and a person on a Chinese Chrome who chose English got every
 * request window — the signing sheet included — in Chinese. The worker now
 * opens this page, which reads the pinned language where the wallet keeps it
 * and asks `surfaceLocale`, the one rule the side panel's doorway (`panel.js`)
 * asks too.
 *
 * Bundled by build.mjs like the other page-side scripts. Runs in an extension
 * page, so `chrome.*` is available.
 */
/* global chrome */
import {
	INSTALLED_KEY,
	installedMark,
	openDoorTarget,
	pinnedLanguage,
	surfaceLocale
} from './lib/locales.js';

const locale = surfaceLocale(pinnedLanguage(), chrome.i18n?.getUILanguage?.());
// A fresh install with web pages already open (spec 094 S3): the welcome this
// tab lands on says to reload them. Carried in the TAB's session storage,
// which survives the navigation below and goes with the tab.
if (installedMark(location.search)) {
	try {
		sessionStorage.setItem(INSTALLED_KEY, '1');
	} catch {
		/* storage denied: the welcome simply says nothing about other tabs */
	}
}
location.replace(chrome.runtime.getURL(openDoorTarget(location.search, locale)));
