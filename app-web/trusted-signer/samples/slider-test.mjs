// The slider, dragged — by real input events, on a real touch screen.
//
// Every other sample confirms with `window.__slider.__confirm()`, which is a
// door only automation has. So the ONE control a person uses to accept a
// signature has never been driven by an event in any test, and the owner found
// that out on a phone: 「创建钱包时，滑动签名不可用，无法滑动」.
//
// This drives it the way a thumb does — CDP touch events, with mobile
// emulation on — for a SIGNATURE and for a CEREMONY, over the url channel,
// which is the only channel the clients still have (spec 076). And it checks
// the knob actually moved, not merely that something fired: a knob that does
// not follow the finger is unusable even when the confirm eventually lands.
//
//   CHROME_BIN=… SB=… bun samples/slider-test.mjs
import { createHash } from 'node:crypto';
import { Page, b64url, makeChecks, sleep, startBrowser } from './test-kit.mjs';

const CDP = 9397;
const TLS_PORT = 8449;
const SIGNER = 'https://getvela.app/src/sign.html';
const check = makeChecks();

const browser = await startBrowser({ cdp: CDP, tlsPort: TLS_PORT, hosts: ['getvela.app'] });

/** A phone: touch events, a phone's width, and a device pixel ratio. */
async function asPhone(page) {
  await page.send('Emulation.setDeviceMetricsOverride', {
    width: 390, height: 844, deviceScaleFactor: 3, mobile: true,
  });
  await page.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
  await page.send('Emulation.setEmitTouchEventsForMouse', { enabled: true, configuration: 'mobile' });
}

/** Where the knob is now, and how far it can go. */
const geometry = (page) => page.ev(`(() => {
  const track = document.querySelector('.slide');
  const knob = document.querySelector('.slide-knob');
  if (!track || !knob) return null;
  const t = track.getBoundingClientRect();
  const k = knob.getBoundingClientRect();
  return {
    off: track.classList.contains('slide-off'),
    knob: { x: k.x + k.width / 2, y: k.y + k.height / 2, w: k.width },
    track: { x: t.x, w: t.width },
    travel: Math.round(k.x - t.x),
  };
})()`);

/**
 * Drag the knob to the far end with TOUCH events, in steps, as a thumb does.
 *
 * Reports where the knob got to at the halfway point, because that is the
 * difference between "the control works" and "the control is dead but something
 * confirmed at the end".
 */
async function dragWithTouch(page, { steps = 12 } = {}) {
  const start = await geometry(page);
  if (!start) return { start: null };
  // What the knob actually receives, and what the slider makes of it. When a
  // drag does nothing, this is the difference between "no pointerup arrived"
  // and "release() decided the travel was not far enough".
  await page.ev(`(() => {
    window.__ev = [];
    const knob = document.querySelector('.slide-knob');
    const track = document.querySelector('.slide');
    for (const type of ['pointerdown', 'pointermove', 'pointerup', 'pointercancel',
      'lostpointercapture', 'touchend', 'touchcancel', 'click']) {
      knob.addEventListener(type, () => window.__ev.push(type), true);
      if (type === 'touchend' || type === 'touchcancel') {
        document.addEventListener(type, () => window.__ev.push('doc:' + type), true);
      }
    }
    window.__span = track.clientWidth - knob.offsetWidth - 12;
    return true;
  })()`);
  const y = start.knob.y;
  const from = start.knob.x;
  const to = start.track.x + start.track.w - start.knob.w / 2 - 6;
  const touch = (type, x) => page.send('Input.dispatchTouchEvent', {
    type,
    touchPoints: type === 'touchEnd' ? [] : [{ x, y, radiusX: 12, radiusY: 12, force: 1 }],
  });
  await touch('touchStart', from);
  let midway = null;
  for (let i = 1; i <= steps; i += 1) {
    await touch('touchMove', from + ((to - from) * i) / steps);
    await sleep(16);
    if (i === Math.ceil(steps / 2)) midway = await geometry(page);
  }
  const end = await geometry(page);
  await touch('touchEnd', to);
  await sleep(400);
  const events = await page.ev('window.__ev ? window.__ev.join(" ") : ""');
  const span = await page.ev('window.__span');
  return { start, midway, end, events, span };
}

// --- 1. a signature, dragged ------------------------------------------------
{
  const intent = {
    method: 'personal_sign',
    params: ['0x' + Buffer.from('hello from the slider test').toString('hex'),
      '0x88cCA0EeDbF2C4426110bbFc998F048689266894'],
    origin: 'https://app.example',
  };
  const context = {
    chainId: 100, account: '0x88cCA0EeDbF2C4426110bbFc998F048689266894',
    accountName: 'Mine', appName: 'Vela Wallet', keys: [],
  };
  const payload = JSON.stringify({ intent, context });
  const page = await Page.open(CDP, `${SIGNER}?ch=url&lang=en#i=${b64url(payload)}&cb=${b64url('velawallet://sign-result')}&t=slide-1`);
  await asPhone(page);
  await page.addAuthenticator();
  const drawn = await page.waitFor("!!document.querySelector('.slide')");
  check('a signature over the url channel draws a card with a slider', drawn,
    drawn ? '' : 'state=' + await page.ev('window.__velaState && JSON.stringify(window.__velaState)')
      + ' text=' + String(await page.text()).replace(/\n/g, ' / ').slice(0, 200));
  await sleep(600);

  const dragged = await dragWithTouch(page);
  check('a signature card has a slider at all', !!dragged.start, JSON.stringify(dragged.start));
  check('the slider is not drawn off for a personal_sign', dragged.start && !dragged.start.off);
  check('the knob FOLLOWS a thumb: it has moved by the halfway point',
    dragged.midway && dragged.midway.travel > 20, `travel=${dragged.midway && dragged.midway.travel}px`);
  console.log('    events:', dragged.events, '| span:', dragged.span, '| knob transform travel:', dragged.end && dragged.end.travel);
  check('the knob reaches the far end of the track',
    dragged.end && dragged.end.travel > (dragged.end.track.w - dragged.end.knob.w - 24),
    `travel=${dragged.end && dragged.end.travel}px of ${dragged.end && Math.round(dragged.end.track.w)}px`);
  // What a confirm DOES, observably and without a key having to exist: the
  // page stops asking to be dragged and goes to the authenticator. (Counting
  // calls is not possible from outside — `release()` calls the function it
  // captured, not the `__confirm` handle automation uses. And asserting the
  // ceremony SUCCEEDS would need a credential this authenticator has none of:
  // that is what `ceremony-test.mjs` is for.)
  const moved = await page.waitFor(`(() => {
    const status = document.getElementById('status');
    return !!status && !/drag/i.test(status.textContent);
  })()`, 6000);
  check('a full drag confirms: the page leaves the drag and goes to the key', moved,
    'status=' + String(await page.status()).slice(0, 80));
  await page.send('Target.closeTarget', { targetId: page.targetId });
}

// --- 2. a create ceremony, dragged ------------------------------------------
//
// The owner's report was about creating a wallet, and a ceremony card is drawn
// by a different branch of the page than a signing card.
{
  const intent = {
    method: 'vela_createPasskey',
    params: [{ name: 'Mine', excludeCredentialIds: [] }],
    origin: '',
  };
  const context = { walletName: 'Mine', appName: 'Vela Wallet', rpId: 'getvela.app' };
  const payload = JSON.stringify({ intent, context });
  const page = await Page.open(CDP, `${SIGNER}?ch=url&lang=en#i=${b64url(payload)}&cb=${b64url('velawallet://sign-result')}&t=slide-2`);
  await asPhone(page);
  await page.addAuthenticator();
  const drawn = await page.waitFor("!!document.querySelector('.slide')");
  check('a create ceremony over the url channel draws a card', drawn,
    String(await page.text()).replace(/\n/g, ' / ').slice(0, 200));
  await sleep(600);

  const dragged = await dragWithTouch(page);
  check('the create card\'s slider is armed, not drawn off',
    dragged.start && !dragged.start.off,
    dragged.start ? `off=${dragged.start.off}` : 'no slider');
  check('the create card\'s knob follows a thumb',
    dragged.midway && dragged.midway.travel > 20, `travel=${dragged.midway && dragged.midway.travel}px`);
  const answered = await page.waitFor("window.__velaState && window.__velaState.answered > 0", 8000);
  check('a full drag on the create card creates the key and answers it', answered,
    'state=' + await page.ev('window.__velaState && JSON.stringify(window.__velaState)'));
  await page.send('Target.closeTarget', { targetId: page.targetId });
}

// --- 3. a create whose answer would go somewhere else ------------------------
//
// The rule the create card now stands on: the answer reaches a Vela wallet.
// Somebody who wants the key has to receive it, so they have to name a callback
// they can receive on — and naming their own is what this refuses.
{
  const intent = {
    method: 'vela_createPasskey', params: [{ name: 'Mine', excludeCredentialIds: [] }], origin: '',
  };
  const context = { walletName: 'Mine', rpId: 'getvela.app' };
  const payload = JSON.stringify({ intent, context });
  const page = await Page.open(CDP, `${SIGNER}?ch=url&lang=en#i=${b64url(payload)}&cb=${b64url('evilwallet://take-it')}&t=slide-3`);
  await asPhone(page);
  await page.addAuthenticator();
  await page.waitFor("!!document.querySelector('.slide')");
  await sleep(600);
  const off = await page.ev("document.querySelector('.slide').classList.contains('slide-off')");
  const text = String(await page.text());
  check('a create answering somewhere that is not a Vela wallet is refused', off,
    text.replace(/\n/g, ' / ').slice(0, 140));
  check('and the card says why, in terms of where the answer goes',
    /answer would not reach one/.test(text));
  check('the card lends the stranger no name and no mark of Vela\'s',
    /Not the Vela wallet/.test(text) && !/Vela wallet on this device/.test(text));
  const dragged = await dragWithTouch(page);
  check('and it cannot be dragged past the refusal',
    dragged.end && dragged.end.travel < 20 &&
      (await page.ev('window.__velaState && window.__velaState.answered')) === 0,
    `travel=${dragged.end && dragged.end.travel}px`);
  await page.send('Target.closeTarget', { targetId: page.targetId });
}

// --- 4. a member proof computes its challenge, and the deployment decides it --
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
    const payload = JSON.stringify({ intent: member(over), context: { walletName: 'Mine', rpId: 'getvela.app' } });
    const page = await Page.open(CDP, `${SIGNER}?ch=url&lang=en#i=${b64url(payload)}&cb=${b64url('velawallet://sign-result')}&t=member-${label}`);
    await asPhone(page);
    await page.addAuthenticator();
    await page.waitFor("!!document.querySelector('.slide')");
    await sleep(400);
    await page.ev("[...document.querySelectorAll('details')].forEach(d => d.open = true)");
    const shown = String(await page.ev("(document.querySelector('.challenge-text') || {}).textContent || ''")).trim();
    const off = await page.ev("document.querySelector('.slide').classList.contains('slide-off')");
    const text = String(await page.text());
    await page.send('Target.closeTarget', { targetId: page.targetId });
    return { shown, off, text };
  };

  const real = await challengeFor({}, 'real');
  check('a member proof draws a card with a challenge, and no network was needed',
    /^0x[0-9a-f]{64}$/.test(real.shown) && !real.off, real.shown.slice(0, 20) + '…');
  check('and it says the challenge was computed here', /computed here/i.test(real.text));

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

browser.kill();
process.exit(check.summary() ? 0 : 1);
