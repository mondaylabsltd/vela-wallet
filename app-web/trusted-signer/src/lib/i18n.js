// Translation, no dependencies.
//
// Nothing in resolve.js produces a human-readable string. It produces keys and
// parameters; only this file turns them into words. That is what makes the
// wording auditable in one place, and a new locale one plain object.
//
// Fifteen locales since spec 102 — the apps' fifteen (vela-core
// `i18n::resolve::SUPPORTED`), each compiled into the page's hashed bytes so a
// language never arrives from anywhere else.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var catalogs = {};
  var current = 'en';

  function register(locale, entries) {
    catalogs[locale] = entries;
  }

  function available() {
    return Object.keys(catalogs);
  }

  /**
   * The shipped locale a language tag means, or null — vela-core's
   * `match_system_tag`, the rule every app uses (spec 095), so the page
   * speaks the language the wallet would.
   *
   * BCP-47 (`zh-Hant-HK`) and POSIX (`zh_TW.UTF-8`) alike. Chinese goes by
   * script first, then region: `Hant` is traditional (Hong Kong and Macau →
   * `zh-HK`, else `zh-TW`), an explicit `Hans` is simplified, an unmarked tag
   * is traditional only for TW, HK or MO. Any `es` is `es-MX`, any `pt` is
   * `pt-BR`, `in` is Indonesian's legacy code. Otherwise language-region if it
   * ships, then the language alone.
   */
  function match(tag) {
    if (typeof tag !== 'string' || !tag) return null;
    var subtags = tag.split(/[.@]/)[0].replace(/_/g, '-').split('-')
      .filter(Boolean)
      .map(function (part) { return part.toLowerCase(); });
    if (!subtags.length) return null;
    var code = subtags[0] === 'in' ? 'id' : subtags[0];
    var rest = subtags.slice(1);
    var script = null;
    var region = null;
    rest.forEach(function (part) {
      if (script === null && part.length === 4) script = part;
      if (region === null && (part.length === 2 || (part.length === 3 && /^\d{3}$/.test(part)))) {
        region = part.toUpperCase();
      }
    });
    var found;
    if (code === 'zh') {
      var traditional = script === 'hant' || (script === null && /^(TW|HK|MO)$/.test(region || ''));
      found = !traditional ? 'zh' : /^(HK|MO)$/.test(region || '') ? 'zh-HK' : 'zh-TW';
    } else if (code === 'es') {
      found = 'es-MX';
    } else if (code === 'pt') {
      found = 'pt-BR';
    } else if (region && catalogs[code + '-' + region]) {
      found = code + '-' + region;
    } else {
      found = code;
    }
    return catalogs[found] ? found : null;
  }

  /**
   * Pick a locale: an explicit `?lang=` → the browser's languages, most
   * preferred first → English (the apps' fallback).
   *
   * Nothing is remembered. A choice kept in this origin's storage outlived
   * the language the person's device is now set to, and every signing page
   * shares one origin.
   */
  function detect(override) {
    var explicit = match(override);
    if (explicit) return explicit;
    var tags = (typeof navigator !== 'undefined' &&
      (navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language])) || [];
    for (var i = 0; i < tags.length; i++) {
      var found = match(String(tags[i] || ''));
      if (found) return found;
    }
    return catalogs.en ? 'en' : available()[0];
  }

  function setLocale(locale) {
    if (!catalogs[locale]) return current;
    current = locale;
    if (typeof document !== 'undefined' && document.documentElement) {
      document.documentElement.setAttribute('lang', locale);
    }
    return current;
  }

  /**
   * t('sentence.transfer', { amount: '1,000 USDC', to: 'Alice Chen' })
   *
   * A missing key returns the key itself rather than an empty string: a hole in
   * a signing prompt must be loud, never invisible.
   */
  function t(key, params) {
    if (key === null || key === undefined) return '';
    if (typeof key === 'object') return t(key.key, key.params);
    var entry = (catalogs[current] && catalogs[current][key]);
    if (entry === undefined) entry = (catalogs.en && catalogs.en[key]);
    if (entry === undefined) return key;
    if (!params) return entry;
    return entry.replace(/\{(\w+)\}/g, function (whole, name) {
      return params[name] === undefined ? whole : String(params[name]);
    });
  }

  ns.i18n = {
    register: register,
    available: available,
    match: match,
    detect: detect,
    setLocale: setLocale,
    get locale() { return current; },
    t: t,
  };
})(window.VelaCS);
