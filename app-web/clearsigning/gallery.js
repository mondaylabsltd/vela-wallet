// The gallery owns no data. It fetches signing intents from outside itself —
// ?src=<url>, or samples/intents.json — and runs each one through the same
// pipeline the real page uses: intent → resolve → render.
(function (ns) {
  'use strict';

  var params = new URLSearchParams(location.search);
  var source = params.get('src') || 'samples/intents.json';
  var grid = document.getElementById('grid');
  var cases = [];

  var COPY = {
    zh: { title: '清晰签名 · 渲染样例', sub: '{n} 个场景，意图全部来自外部数据（{src}）', level: '降级 {n} 级 · {risk}', fail: '渲染失败', load: '读不到 {src} —— 用 http 打开这一页，或用 ?src= 指定一个意图 JSON。' },
    en: { title: 'Clear Signing · rendered examples', sub: '{n} scenarios, every intent loaded from outside ({src})', level: 'level {n} · {risk}', fail: 'render failed', load: 'Cannot read {src} — serve this page over http, or point ?src= at an intents JSON.' },
  };

  function copy(key, params2) {
    var table = COPY[ns.i18n.locale] || COPY.en;
    return (table[key] || key).replace(/\{(\w+)\}/g, function (whole, name) {
      return params2 && params2[name] !== undefined ? params2[name] : whole;
    });
  }

  // The JSON carries integers as strings — JSON has no bigint.
  function reviveContext(context) {
    var ctx = Object.assign({}, context);
    if (typeof ctx.currentAllowance === 'string') ctx.currentAllowance = BigInt(ctx.currentAllowance);
    return ctx;
  }

  function draw() {
    grid.innerHTML = '';
    document.getElementById('gallery-title').textContent = copy('title');
    document.getElementById('gallery-sub').textContent = copy('sub', { n: cases.length, src: source });

    cases.forEach(function (entry) {
      var wrap = document.createElement('section');
      wrap.className = 'case';

      var title = document.createElement('p');
      title.className = 'case-title';
      var code = document.createElement('span');
      code.className = 'case-code';
      code.textContent = entry.code;
      title.appendChild(code);
      var name = entry.title && (entry.title[ns.i18n.locale] || entry.title.en || entry.title);
      title.appendChild(document.createTextNode(typeof name === 'string' ? name : ''));
      wrap.appendChild(title);

      try {
        var view = ns.resolve(entry.intent, reviveContext(entry.context));
        var level = document.createElement('span');
        level.className = 'case-level';
        level.textContent = copy('level', { n: view.level, risk: view.risk });
        title.appendChild(level);
        wrap.appendChild(ns.render(view, entry.options));
      } catch (error) {
        var failed = document.createElement('pre');
        failed.className = 'block';
        failed.textContent = copy('fail') + ': ' + error.message + '\n' + error.stack;
        wrap.appendChild(failed);
      }
      grid.appendChild(wrap);
    });

    drawSelfCheck();
  }

  // Every descriptor's selector, recomputed from its signature. If one ever
  // disagrees, a descriptor is claiming a function it does not have.
  function drawSelfCheck() {
    var proof = ns.registry.selectorProof;
    var good = proof.filter(function (e) { return ns.keccak.selector(e.signature) === e.selector; }).length;
    var box = document.getElementById('selfcheck');
    box.innerHTML = '';
    var head = document.createElement('div');
    head.textContent = 'selector self-check: ' + good + '/' + proof.length +
      ' match keccak256(signature) · keccak vector: ' +
      (ns.keccak.hex(new TextEncoder().encode('')) ===
        '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470' ? 'pass' : 'FAIL');
    box.appendChild(head);
    proof.forEach(function (entry) {
      var row = document.createElement('div');
      row.textContent = entry.selector + '  ' + entry.signature;
      box.appendChild(row);
    });
  }

  // --- chrome ---------------------------------------------------------------

  var select = document.getElementById('lang');
  ns.i18n.available().forEach(function (locale) {
    var option = document.createElement('option');
    option.value = locale;
    option.textContent = ns.i18n.t('locale.name');
    select.appendChild(option);
  });
  ns.i18n.setLocale(ns.i18n.detect(params.get('lang')));
  Array.prototype.forEach.call(select.options, function (option) {
    var saved = ns.i18n.locale;
    ns.i18n.setLocale(option.value);
    option.textContent = ns.i18n.t('locale.name');
    ns.i18n.setLocale(saved);
  });
  select.value = ns.i18n.locale;
  select.addEventListener('change', function () {
    ns.i18n.setLocale(select.value);
    draw();
  });

  var theme = document.getElementById('theme');
  theme.addEventListener('click', function () {
    var now = document.documentElement.getAttribute('data-theme');
    var next = now === 'dark' ? 'light' : now === 'light' ? '' : 'dark';
    if (next) document.documentElement.setAttribute('data-theme', next);
    else document.documentElement.removeAttribute('data-theme');
    theme.textContent = next === 'dark' ? '●' : next === 'light' ? '○' : '◐';
  });

  fetch(source)
    .then(function (response) {
      if (!response.ok) throw new Error(response.status + ' ' + response.statusText);
      return response.json();
    })
    .then(function (payload) {
      cases = payload.cases || [];
      draw();
    })
    .catch(function () {
      document.getElementById('gallery-sub').textContent = copy('load', { src: source });
      drawSelfCheck();
    });
})(window.VelaCS);
