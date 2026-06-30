/**
 * options.js — Settings page for the WebAuthn proxy.
 *
 * Lets the user pick which production domain's passkeys to proxy, and requests
 * the matching host permission (declared as optional_host_permissions in the
 * manifest). The chosen domain is stored in chrome.storage.sync.proxyRpId,
 * which bridge.js relays into the page and webauthn.js asserts as the rpId.
 */

const DEFAULT_RP_ID = 'getvela.app';
const $ = (id) => document.getElementById(id);

/** Strip scheme/port/path if the user pasted a full URL. */
function normalizeDomain(input) {
  let d = (input || '').trim().toLowerCase();
  if (!d) return '';
  d = d.replace(/^[a-z][a-z0-9+.-]*:\/\//, ''); // scheme://
  d = d.replace(/\/.*$/, '');                    // path
  d = d.replace(/:\d+$/, '');                    // :port
  return d;
}

const pattern = (domain) => `*://${domain}/*`;

/**
 * The dev/preview host patterns the extension injects into, read straight from
 * the manifest's content_scripts. Port wildcards are stripped (match patterns
 * already cover every port) and the result deduped, so they're valid origins
 * for chrome.permissions.request.
 */
function devHostPatterns() {
  const cs = chrome.runtime.getManifest().content_scripts || [];
  const raw = cs.flatMap((s) => s.matches || []);
  const cleaned = raw.map((p) => p.replace(/:(\*|\d+)(?=\/)/, ''));
  return [...new Set(cleaned)];
}

async function refreshDevHosts() {
  const origins = devHostPatterns();
  let granted = 0;
  for (const o of origins) {
    if (await chrome.permissions.contains({ origins: [o] })) granted++;
  }
  const all = granted === origins.length;
  $('devStatus').textContent = all
    ? `✓ All ${origins.length} dev/preview hosts allowed.`
    : `${granted} of ${origins.length} dev/preview hosts allowed — click below to grant the rest.`;
  $('devStatus').className = 'status ' + (all ? 'ok' : granted ? 'warn' : '');
}

async function refresh() {
  const { proxyRpId } = await chrome.storage.sync.get({ proxyRpId: DEFAULT_RP_ID });
  $('domain').value = proxyRpId;

  const has = await chrome.permissions.contains({ origins: [pattern(proxyRpId)] });
  $('status').textContent = has
    ? `✓ Access granted for ${proxyRpId}.`
    : `⚠ No site access for ${proxyRpId} yet — click “Save & grant access”.`;
  $('status').className = 'status ' + (has ? 'ok' : 'warn');

  const all = await chrome.permissions.getAll();
  const origins = all.origins || [];
  $('granted').textContent = origins.length ? 'Granted host access: ' + origins.join('  ·  ') : '';
}

$('save').addEventListener('click', async () => {
  const domain = normalizeDomain($('domain').value);
  if (!domain) {
    $('status').textContent = 'Enter a domain first.';
    $('status').className = 'status warn';
    return;
  }

  // permissions.request must run in a user gesture — this click qualifies.
  const granted = await chrome.permissions.request({ origins: [pattern(domain)] });
  if (!granted) {
    $('status').textContent = `Permission denied for ${domain}. WebAuthn won't work until you grant it.`;
    $('status').className = 'status warn';
    return;
  }

  await chrome.storage.sync.set({ proxyRpId: domain });
  await refresh();
  $('status').textContent = `✓ Saved ${domain}. Reload your dev page to apply.`;
  $('status').className = 'status ok';
});

$('grantDev').addEventListener('click', async () => {
  const origins = devHostPatterns();
  // One gesture, one combined Chrome prompt for every dev/preview host.
  const granted = await chrome.permissions.request({ origins });
  if (!granted) {
    $('devStatus').textContent = 'Permission denied — dev hosts not granted.';
    $('devStatus').className = 'status warn';
    return;
  }
  await refresh();
  await refreshDevHosts();
  $('devStatus').textContent = '✓ All dev/preview hosts granted. Reload your dev page to apply.';
  $('devStatus').className = 'status ok';
});

refresh();
refreshDevHosts();
