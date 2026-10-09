// The button, tapped — by real input events, on a real touch screen.
//
// Every other sample confirms with `window.__slider.__confirm()`, which is a
// door only automation has. So the ONE control a person uses to accept a
// signature is driven here the way a thumb does: CDP touch events, with mobile
// emulation on. Since spec 102 that control is a tap (it was a slide; every
// confirmation in Vela is a tap now), and a tap has one thing a slide did not
// have to prove: the passkey prompt must start INSIDE the tap's own event,
// because Safari grants it only to code running in the person's gesture. So
// this wraps `navigator.credentials` before the page loads and records, at the
// moment of each call, whether the page still had the person's activation —
// and what it asked for (the key route, R5).
//
//   CHROME_BIN=… SB=… bun samples/confirm-test.mjs
import { webcrypto } from 'node:crypto';
import { Page, b64url, makeChecks, sleep, startBrowser } from './test-kit.mjs';

const CDP = 9397;
const TLS_PORT = 8449;
const SIGNER = 'https://getvela.app/src/sign.html';
const WALLET = 'velawallet://sign-result';
const check = makeChecks();

const browser = await startBrowser({ cdp: CDP, tlsPort: TLS_PORT, hosts: ['getvela.app'] });

// Installed before any of the page's own scripts: every WebAuthn call, with
// whether it ran inside the person's gesture, and what it asked for. During
// the call it also taps the button AGAIN, as an impatient thumb would — a
// second prompt must not follow.
const RECORDER = `(() => {
  const calls = window.__webauthn = [];
  const c = navigator.credentials;
  const get = c.get.bind(c);
  const create = c.create.bind(c);
  const b64 = (v) => btoa(String.fromCharCode(...new Uint8Array(v.buffer ? v.buffer.slice(v.byteOffset, v.byteOffset + v.byteLength) : v)))
    .replace(/\\+/g, '-').replace(/\\//g, '_').replace(/=+$/, '');
  const record = (kind, options) => {
    const pk = (options && options.publicKey) || {};
    const button = document.querySelector('.confirm');
    calls.push({
      kind,
      activated: !!(navigator.userActivation && navigator.userActivation.isActive),
      busy: !!(button && button.classList.contains('busy')),
      hints: pk.hints || null,
      attachment: (pk.authenticatorSelection && pk.authenticatorSelection.authenticatorAttachment) || null,
      allow: (pk.allowCredentials || []).map((a) => ({ id: b64(a.id), transports: a.transports || null })),
    });
    if (button) button.click();
  };
  c.get = (options) => { record('get', options); return get(options); };
  c.create = (options) => { record('create', options); return create(options); };
})();`;

/** A phone, with the recorder in place, at `url`. */
async function phoneAt(url, { key } = {}) {
  const page = await Page.open(CDP);
  await page.send('Page.enable');
  await page.send('Page.addScriptToEvaluateOnNewDocument', { source: RECORDER });
  await page.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 3, mobile: true });
  await page.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
  await page.addAuthenticator();
  if (key) {
    await page.send('WebAuthn.addCredential', {
      authenticatorId: page.authenticatorId,
      credential: {
        credentialId: Buffer.from(key.rawId).toString('base64'), isResidentCredential: true, rpId: 'getvela.app',
        privateKey: key.pkcs8.toString('base64'), userHandle: Buffer.from('vela-test').toString('base64'), signCount: 0,
      },
    });
  }
  await page.navigate(url);
  return page;
}

async function newKey() {
  const pair = await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
  const rawId = Buffer.from(webcrypto.getRandomValues(new Uint8Array(16)));
  return { rawId, id: rawId.toString('base64url'), pkcs8: Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey)) };
}

/** A thumb on the button: touchStart and touchEnd at its centre. */
async function tap(page) {
  const box = await page.ev(`(() => {
    const b = document.querySelector('.confirm');
    if (!b) return null;
    b.scrollIntoView({ block: 'center' });
    const r = b.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);
  if (!box) return false;
  await sleep(120);
  const point = [{ x: box.x, y: box.y, radiusX: 8, radiusY: 8, force: 1, id: 1 }];
  await page.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: point });
  await sleep(60);
  await page.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  return true;
}

const fragment = (intent, context, cb, token) =>
  `#i=${b64url(JSON.stringify({ intent, context }))}&cb=${b64url(cb)}&t=${token}`;
const SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const hello = '0x' + Buffer.from('hello from the confirm test').toString('hex');

// --- 1. a signature, tapped, with the key route ------------------------------
{
  const key = await newKey();
  const intent = { method: 'personal_sign', params: [hello, SAFE], origin: 'https://app.example' };
  const context = {
    chainId: 100, account: SAFE, allowCredentials: [key.id],
    keyRoute: { credentialId: key.id, place: 'platform', transports: ['internal', 'hybrid'], hints: ['client-device'] },
  };
  const page = await phoneAt(`${SIGNER}?ch=url&lang=en${fragment(intent, context, WALLET, 'tap-1')}`, { key });
  const drawn = await page.waitFor("!!window.__slider && !!document.querySelector('.confirm')");
  check('a signature over the url channel draws a card with a button', drawn,
    drawn ? '' : String(await page.text()).replace(/\n/g, ' / ').slice(0, 200));
  const label = await page.ev("document.querySelector('.confirm').textContent");
  check('the button says the action', label === 'Sign', label);
  check('it is armed', await page.ev("!document.querySelector('.confirm').disabled"));
  check('the card says where the person confirms',
    /This device/.test(await page.ev("(document.querySelector('.key-row') || {}).textContent || ''")));
  await sleep(300);

  await tap(page);
  const answered = await page.waitFor('window.__velaState.answered > 0 && !!window.__result', 8000);
  const calls = await page.ev('window.__webauthn');
  check('a tap signs: the key answered and the page answered the wallet', answered,
    'state=' + await page.ev('JSON.stringify(window.__velaState)'));
  check('the passkey prompt started INSIDE the tap (Safari\'s rule)', calls.length >= 1 && calls[0].activated === true,
    JSON.stringify(calls[0]));
  check('…while the button showed it was busy', calls[0] && calls[0].busy === true);
  check('a second tap while the prompt was up asked nothing more', calls.length === 1, `${calls.length} calls`);
  check('the request named the one key, with its transports and the device hint',
    calls[0] && calls[0].allow.length === 1 && calls[0].allow[0].id === key.id &&
    JSON.stringify(calls[0].allow[0].transports) === '["internal","hybrid"]' &&
    JSON.stringify(calls[0].hints) === '["client-device"]', JSON.stringify(calls[0]));
  check('the key that answered is the routed one', (await page.ev('window.__result.credentialId')) === key.id);
  const after = await page.ev("(() => { const b = document.querySelector('.confirm'); return { off: b.disabled, text: b.textContent }; })()");
  check('afterwards the button is done, not live', after.off && after.text === 'Done', JSON.stringify(after));
  await page.close();
}

// --- 2. no key on this device: a cancelled prompt leaves the card usable -------
{
  const intent = { method: 'personal_sign', params: [hello, SAFE], origin: '' };
  const context = { chainId: 100, account: SAFE, allowCredentials: [b64url(Buffer.alloc(16, 7))] };
  const page = await phoneAt(`${SIGNER}?ch=url&lang=en${fragment(intent, context, WALLET, 'tap-2')}`);
  await page.waitFor('!!window.__slider');
  await sleep(300);
  await tap(page);
  const back = await page.waitFor("window.__velaState.phase === 'card' && (window.__webauthn || []).length === 1 && /try again/.test(document.getElementById('status').textContent)", 15000);
  check('when no key answers, the page says so and the button is armed again', back,
    'status=' + await page.status());
  check('…not busy, not disabled', await page.ev("(() => { const b = document.querySelector('.confirm'); return !b.disabled && !b.classList.contains('busy'); })()"));
  check('no route was sent: no hints, no transports (an older wallet\'s request, unchanged)',
    await page.ev('window.__webauthn[0].hints === null && window.__webauthn[0].allow.every((a) => a.transports === null)'));
  await page.close();
}

// --- 3. a create ceremony, tapped, on the place the person chose ---------------
for (const [place, attachment, hint] of [['platform', 'platform', 'client-device'], ['security_key', 'cross-platform', 'security-key']]) {
  const intent = {
    method: 'vela_createPasskey',
    params: [{ name: 'Mine', excludeCredentialIds: [], place, hints: [hint] }],
    origin: '',
  };
  const page = await phoneAt(`${SIGNER}?ch=url&lang=en${fragment(intent, { walletName: 'Mine' }, WALLET, 'tap-3-' + place)}`);
  await page.waitFor('!!window.__slider');
  check(`create on ${place}: the button says "Create key"`, (await page.ev("document.querySelector('.confirm').textContent")) === 'Create key');
  await sleep(300);
  await tap(page);
  const calls = (await page.waitFor('(window.__webauthn || []).length > 0', 8000)) ? await page.ev('window.__webauthn') : [];
  check(`create on ${place}: the prompt started inside the tap`, calls[0] && calls[0].kind === 'create' && calls[0].activated === true,
    JSON.stringify(calls[0]));
  check(`create on ${place}: it asked for ${attachment} with the ${hint} hint`,
    calls[0] && calls[0].attachment === attachment && JSON.stringify(calls[0].hints) === JSON.stringify([hint]), JSON.stringify(calls[0]));
  if (place === 'platform') {
    // The virtual authenticator is a platform one: it can make this key.
    check('create on platform: the key was made and handed back',
      await page.waitFor('window.__velaState.answered > 0', 8000), 'state=' + await page.ev('JSON.stringify(window.__velaState)'));
  }
  await page.close();
}

// --- 4. a create whose answer would go somewhere else --------------------------
//
// The rule the create card stands on: the answer reaches a Vela wallet.
// Somebody who wants the key has to receive it, so they have to name a callback
// they can receive on — and naming their own is what this refuses.
{
  const intent = { method: 'vela_createPasskey', params: [{ name: 'Mine', excludeCredentialIds: [] }], origin: '' };
  const page = await phoneAt(`${SIGNER}?ch=url&lang=en${fragment(intent, { walletName: 'Mine' }, 'evilwallet://take-it', 'tap-4')}`);
  await page.waitFor("!!document.querySelector('.confirm')");
  await sleep(400);
  const button = await page.ev("(() => { const b = document.querySelector('.confirm'); return { off: b.disabled, text: b.textContent }; })()");
  const text = String(await page.text());
  check('a create answering somewhere that is not a Vela wallet is refused', button.off,
    text.replace(/\n/g, ' / ').slice(0, 140));
  check('the button says it cannot sign, where the person expects it', button.text === 'Can’t sign this', button.text);
  check('and the card says why, in terms of where the answer goes', /answer would not reach one/.test(text));
  check('the card lends the stranger no name and no mark of Vela\'s',
    /Not the Vela wallet/.test(text) && !/Vela wallet on this device/.test(text));
  await tap(page);
  await sleep(400);
  check('a tap on it does nothing: no prompt, no answer',
    (await page.ev('(window.__webauthn || []).length')) === 0 && (await page.ev('window.__velaState.answered')) === 0);
  await page.close();
}

// --- 5. a member proof computes its challenge, and the deployment decides it ---
//
// The page used to FETCH this challenge. It cannot: the published page carries
// `default-src 'none'` in its hashed bytes (076), and the owner found that out
// the hard way — creating a wallet failed at its second step with 「注册表没有
// 应答」 (2026-09-24). So the wallet names the chain and the registry contract,
// and the page computes the challenge from them and from the keys on screen.
//
// Which makes the deployment load-bearing, and that is what this checks: change
// either fact and the bytes signed change. The wallet compares what comes back
// against the challenge IT fetched, so a requester that lies gets refused —
// that half is asserted where a real core can do the refusing (Rust, Android,
// iOS).
{
  const member = (over) => ({
    method: 'vela_memberProof',
    params: [{
      // 24 bytes: the page requires 16 or more, base64url.
      credentialId: b64url(Buffer.from('a-credential-of-24-bytes')),
      publicKey: '04' + '11'.repeat(64),
      groupPublicKey: '04' + '22'.repeat(64),
      attestation: '',
      registry: 'https://p256-index-v2.getvela.app',
      chainId: 100,
      registryContract: '0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf',
      ...over,
    }],
    origin: '',
  });

  /** The challenge the card shows for these facts, or null when it refuses. */
  const challengeFor = async (over, label) => {
    const page = await phoneAt(`${SIGNER}?ch=url&lang=en${fragment(member(over), { walletName: 'Mine' }, WALLET, 'member-' + label)}`);
    await page.waitFor("!!document.querySelector('.confirm')");
    await sleep(400);
    await page.ev("[...document.querySelectorAll('details')].forEach(d => d.open = true)");
    const shown = String(await page.ev("(document.querySelector('.challenge-text') || {}).textContent || ''")).trim();
    const off = await page.ev("document.querySelector('.confirm').disabled");
    const text = String(await page.text());
    await page.close();
    return { shown, off, text };
  };

  const real = await challengeFor({}, 'real');
  check('a member proof draws a card with a challenge, and no network was needed',
    /^0x[0-9a-f]{64}$/.test(real.shown) && !real.off, real.shown.slice(0, 20) + '…');
  check('and it says the challenge was computed here', /computed here/i.test(real.text));
  check('and its sentence no longer says the registry was asked', !/asked the registry/i.test(real.text));

  const otherChain = await challengeFor({ chainId: 1 }, 'chain');
  check('another chain gives another challenge', /^0x[0-9a-f]{64}$/.test(otherChain.shown) &&
    otherChain.shown !== real.shown, otherChain.shown.slice(0, 20) + '…');

  const otherContract = await challengeFor({ registryContract: '0x' + '11'.repeat(20) }, 'contract');
  check('another registry contract gives another challenge',
    /^0x[0-9a-f]{64}$/.test(otherContract.shown) && otherContract.shown !== real.shown,
    otherContract.shown.slice(0, 20) + '…');

  const blind = await challengeFor({ chainId: undefined, registryContract: undefined }, 'blind');
  check('and with neither fact the page refuses instead of signing something unchecked',
    blind.off && /which chain and registry/i.test(blind.text), blind.text.replace(/\n/g, ' / ').slice(0, 120));
}

// --- 6. a keyboard works too: Enter on the focused button --------------------
{
  const key = await newKey();
  const intent = { method: 'personal_sign', params: [hello, SAFE], origin: '' };
  const context = { chainId: 100, account: SAFE, allowCredentials: [key.id] };
  const page = await phoneAt(`${SIGNER}?ch=url&lang=en${fragment(intent, context, WALLET, 'tap-6')}`, { key });
  await page.waitFor('!!window.__slider');
  await page.ev("document.querySelector('.confirm').focus()");
  // `text` is what makes it a key that types: Chrome activates a focused
  // button on Enter's keypress, which a keyDown without text never sends.
  await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' });
  await page.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  check('Enter on the focused button signs, inside the key press',
    (await page.waitFor('window.__velaState.answered > 0', 8000)) && (await page.ev('window.__webauthn[0].activated')) === true);
  await page.close();
}

browser.kill();
process.exit(check.summary() ? 0 : 1);
