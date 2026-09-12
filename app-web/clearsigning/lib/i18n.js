// Translation, ~50 lines, no dependencies.
//
// Nothing in resolve.js produces a human-readable string. It produces keys and
// parameters; only this file turns them into words. That is what makes the
// wording auditable in one place, and a new locale one plain object.
window.VelaCS = window.VelaCS || {};
(function (ns) {
  'use strict';

  var catalogs = {};
  var current = 'zh';

  function register(locale, entries) {
    catalogs[locale] = entries;
  }

  function available() {
    return Object.keys(catalogs);
  }

  /** Pick a locale: explicit override → stored choice → browser → zh. */
  function detect(override) {
    if (override && catalogs[override]) return override;
    try {
      var stored = localStorage.getItem('clearsigning.locale');
      if (stored && catalogs[stored]) return stored;
    } catch (e) { /* storage can be blocked */ }
    var tags = (navigator.languages || [navigator.language || 'zh']).map(String);
    for (var i = 0; i < tags.length; i++) {
      var tag = tags[i].toLowerCase();
      if (catalogs[tag]) return tag;
      var base = tag.split('-')[0];
      if (catalogs[base]) return base;
    }
    return 'zh';
  }

  function setLocale(locale) {
    if (!catalogs[locale]) return current;
    current = locale;
    try {
      localStorage.setItem('clearsigning.locale', locale);
    } catch (e) { /* ignore */ }
    document.documentElement.setAttribute('lang', locale);
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
    detect: detect,
    setLocale: setLocale,
    get locale() { return current; },
    t: t,
  };
})(window.VelaCS);
