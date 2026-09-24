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
 * Bundled by build.mjs like the other page-side scripts. Runs in an extension
 * page, so `chrome.*` is available; `chrome.i18n.getUILanguage()` is the same
 * fact the service worker negotiates the wallet tab from.
 */
/* global chrome */
import { negotiate, walletPage } from './lib/locales.js';

const locale = negotiate(chrome.i18n?.getUILanguage?.());
location.replace(`${chrome.runtime.getURL(walletPage(locale))}?panel`);
