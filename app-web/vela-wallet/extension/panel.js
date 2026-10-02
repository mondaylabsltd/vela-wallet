/**
 * The side panel's doorway: pick the locale, open the WALLET.
 *
 * Spec 077 FR-001. This used to go to the request page, so the panel WAS one
 * request and the wallet was not beneath it — it had been replaced. The owner's
 * report: "侧边栏的钱包 UI 都在，签名弹框是类似于 android ios 那样弹出来的".
 *
 * `?panel` is how the wallet knows it is here rather than in an ordinary tab:
 * only the panel answers a dApp's pending request, and only the panel may be
 * dismissed by the worker. A tab is the same page with nobody to answer for.
 *
 * The locale is the wallet's PINNED language when there is one (spec 082 G59:
 * 简体中文 pinned in the panel's Settings, and the reopened panel was English),
 * else Chrome's UI language — `surfaceLocale`, the rule every surface opens
 * by (the request window and the wallet tab go through `open.html`, issue
 * 317). This page shares the wallet's origin, so it reads the same
 * `localStorage` the preferences store writes.
 *
 * Bundled by build.mjs like the other page-side scripts. Runs in an extension
 * page, so `chrome.*` is available.
 */
/* global chrome */
import { pinnedLanguage, surfaceLocale, walletPage } from './lib/locales.js';

const locale = surfaceLocale(pinnedLanguage(), chrome.i18n?.getUILanguage?.());
location.replace(`${chrome.runtime.getURL(walletPage(locale))}?panel`);
