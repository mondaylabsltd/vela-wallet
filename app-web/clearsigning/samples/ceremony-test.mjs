// The page as a passkey route (spec 075), end to end.
//
//   A. the challenges the page derives (Node): the sign-in and proof forms,
//      and the member challenge against answers from the LIVE registry;
//   B. the loopback WebSocket, as the phones run it: one session carrying
//      create → member proof → bye, then another carrying sign-in → verify →
//      recover ×2 → a close without bye, then the idle end;
//   C. postMessage from a getvela.app web wallet: create → sign-in → bye;
//      end to end, the wallet dropping out and coming back, bye.
//
// Every passkey ceremony is real WebAuthn against a CDP virtual authenticator;
// the key the page creates is the key every later assertion must verify under,
// and vela-core (wasm) reads the attestation and recovers the key from the two
// recovery assertions, as the wallet would.
//
//   CHROME_BIN=… SB=… node samples/ceremony-test.mjs
import { webcrypto } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import {
  Page, b64url, clientData, fromHex, isBareHex, loadPageLibs, loopbackWallet, makeChecks,
  root, sleep, startBrowser, unb64url, verifies,
} from './test-kit.mjs';
import { startRegistry } from './mock-registry.mjs';

const CDP = 9392;
const TLS_PORT = 8446;
const SIGNER = 'https://getvela.app/src/sign.html';
const WALLET = 'Test Wallet';
const check = makeChecks();

const ns = loadPageLibs(['src/lib/keccak.js', 'src/lib/abi.js', 'src/lib/encode.js', 'src/lib/signer.js',
  'src/lib/ceremony.js']);

// vela-core, compiled: what the wallet itself does with a registration and
// with two recovery assertions.
const repo = join(root, '..', '..');
const core = await import(join(repo, 'rust/pkg-web/vela_core.js'));
const wasmDir = join(repo, 'assets/wasm');
core.initSync({ module: readFileSync(join(wasmDir, readdirSync(wasmDir).find((n) => /^vela_core_bg\..*\.wasm$/.test(n)))) });

const text = (hex) => Buffer.from(fromHex(hex)).toString('utf8');
// The wallet's own judge (spec 075 T003): vela-core, over the operation the
// machine asked for, exactly as a shell hands it over.
const credHex = (b64) => Buffer.from(unb64url(b64)).toString('hex');
const judge = (operation, answer, expected) =>
  JSON.parse(core.clearSignerVerifyCeremony(JSON.stringify(operation), JSON.stringify(answer), 'https://getvela.app', expected));
const challengeText = (answer) => Buffer.from(unb64url(clientData(answer).challenge)).toString('utf8');
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function answerShape(answer, id, kind) {
  const body = answer[kind];
  const fields = kind === 'registration'
    ? ['credentialId', 'attestationObject', 'clientDataJSON', 'authenticatorAttachment', 'transports']
    : ['credentialId', 'signatureDer', 'authenticatorData', 'clientDataJSON', 'userHandle', 'authenticatorAttachment'];
  const bytes = kind === 'registration'
    ? ['attestationObject', 'clientDataJSON']
    : ['signatureDer', 'authenticatorData', 'clientDataJSON'];
  return answer.t === 'result' && answer.id === id && body && answer.result === undefined &&
    answer.origin === 'https://getvela.app' &&
    fields.every((f) => f in body) &&
    bytes.every((f) => isBareHex(body[f])) &&
    /^[A-Za-z0-9_-]+$/.test(body.credentialId) &&
    typeof body.authenticatorAttachment === 'string' &&
    (kind === 'registration' ? typeof body.transports === 'string' : (body.userHandle === null || isBareHex(body.userHandle)));
}

function userVerified(assertion) {
  return (fromHex(assertion.authenticatorData)[32] & 0x04) !== 0;
}

async function p256PublicHex() {
  const pair = await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']);
  return Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey)).toString('hex');
}

// === A. the challenges the page derives =======================================
{
  const live = JSON.parse(readFileSync(join(root, 'samples/member-challenge-vectors.json'), 'utf8'));
  let agree = 0;
  for (const c of live.cases) {
    const binding = ns.ceremony.memberBinding(c.request.groupPublicKey, c.request.attestation || '');
    const challenge = ns.ceremony.memberChallenge({
      chainId: live.chainId, registry: live.domainRegistry, rpId: c.request.rpId,
      publicKey: c.request.publicKey, binding,
    });
    if ('0x' + ns.ceremony._hex(binding) === c.answer.binding &&
      '0x' + ns.ceremony._hex(challenge) === c.answer.challenge) agree++;
  }
  check('derive: the member challenge is the live registry\'s, byte for byte', agree === live.cases.length,
    `${agree}/${live.cases.length} (with and without attestation, two rpIds)`);

  const a = ns.ceremony.signInChallenge();
  const b = ns.ceremony.signInChallenge();
  check('derive: sign-in = "vela-signin-<ms>-<16 hex>", fresh each time',
    /^vela-signin-\d{13}-[0-9a-f]{16}$/.test(a) && a !== b, a);
  check('derive: sign-in with a fixed clock and noise is exactly that string',
    ns.ceremony.signInChallenge(1788750000000, new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8])) === 'vela-signin-1788750000000-0102030405060708');
  check('derive: proofs = the shells\' own forms',
    ns.ceremony.proofChallenge('verify', 5) === 'vela-verify-5' &&
    ns.ceremony.proofChallenge('recover_first', 5) === 'vela-recover-5' &&
    ns.ceremony.proofChallenge('recover_second', 5) === 'vela-recover-5');
  check('derive: an unknown proof purpose has no challenge at all',
    ns.ceremony.proofChallenge('sign', 5) === null && ns.ceremony.proofChallenge('__proto__', 5) === null);
  check('derive: no text challenge is 32 bytes (a Safe digest\'s length)',
    new TextEncoder().encode(a).length !== 32 && new TextEncoder().encode(ns.ceremony.proofChallenge('verify')).length !== 32);
}

const browser = await startBrowser({ cdp: CDP, tlsPort: TLS_PORT });
const registry = await startRegistry({ ns });
try {
  // === B. the loopback WebSocket =============================================
  const page = await Page.open(CDP, 'about:blank');
  await page.addAuthenticator();

  // B1. create → member proof → bye, one session
  let key = null;          // {x, y} of the key the page created
  let credentialId = null;
  {
    const wallet = loopbackWallet({ token: 'tok-create' });
    await wallet.listening;
    await page.navigate(`${SIGNER}?ch=ws&lang=en&s=1#p=${wallet.port}&t=tok-create`);
    check('ws: the page connected and proved itself with the token',
      await Promise.race([wallet.hello, sleep(8000).then(() => false)]));

    const created = wallet.request('c1', {
      method: 'vela_createPasskey', params: [{ name: WALLET, excludeCredentialIds: [] }], origin: '',
    }, { walletName: WALLET });
    const shown = await page.waitFor("window.__velaState.phase === 'card' && window.__velaState.kind === 'create'");
    const card = await page.text();
    check('create: its own card — title, what it does, the wallet\'s name', shown &&
      card.includes('Create a key') && card.includes(WALLET) && /nothing is signed/i.test(card), card.split('\n')[2]);
    check('create: the card says who asks (the app on this device)', card.includes('the Vela app on this device'));
    await page.ev('window.__slider.__confirm()', true);
    const answer = await created;
    check('create: answered as {t:"result", id, registration, origin} — lowercase hex, no 0x',
      answerShape(answer, 'c1', 'registration'), Object.keys(answer.registration || {}).join(','));
    const reg = answer.registration;
    const cd = clientData(reg);
    check('create: clientData is webauthn.create from https://getvela.app over 32 fresh bytes',
      cd.type === 'webauthn.create' && cd.origin === 'https://getvela.app' && unb64url(cd.challenge).length === 32);
    key = core.extractAttestationPublicKey(fromHex(reg.attestationObject));
    credentialId = reg.credentialId;
    check('create: vela-core reads a P-256 key out of the attestation', /^0x[0-9a-f]{64}$/.test(key.x), key.x.slice(0, 18) + '…');
    check('create: transports are joined with commas', !/[[\]"]/.test(reg.transports), JSON.stringify(reg.transports));
    const judgedCreate = judge({ type: 'register_passkey', name: WALLET, exclude_credential_ids: [], method: 'clear_signer' }, answer);
    check('core: vela-core accepts the registration, as a key living behind https://getvela.app',
      !!judgedCreate.registration && judgedCreate.registration.signer_origin === 'https://getvela.app' &&
      judgedCreate.registration.credential_id === credHex(credentialId), JSON.stringify(judgedCreate).slice(0, 140));

    check('session: after the answer the page is back to "waiting for the wallet"',
      await page.waitFor("window.__velaState.phase === 'waiting'") &&
      /Waiting for the wallet/.test(await page.text()));

    // The member proof: the page asks the registry itself.
    const group = await p256PublicHex();
    const publicKeyHex = '04' + key.x.slice(2) + key.y.slice(2);
    const before = registry.requests.length;
    const member = wallet.request('m1', {
      method: 'vela_memberProof',
      params: [{ credentialId, publicKey: publicKeyHex, attestation: '', groupPublicKey: group, registry: registry.url }],
      origin: '',
    }, { walletName: WALLET });
    const armed = await page.waitFor("window.__velaState.phase === 'card' && window.__velaState.kind === 'memberProof'");
    const asked = registry.requests.slice(before);
    const posted = asked.find((r) => r.path === '/api/challenge');
    check('member: the page fetched /api/health and /api/challenge itself', armed &&
      asked.some((r) => r.path === '/api/health') && !!posted);
    check('member: for exactly the inputs on the card, under its own rpId', !!posted &&
      posted.body.rpId === 'getvela.app' && posted.body.publicKey === publicKeyHex && posted.body.groupPublicKey === group &&
      !('attestation' in posted.body));
    const expected = registry.honestChallenge({ rpId: 'getvela.app', publicKey: publicKeyHex, groupPublicKey: group, attestation: '' });
    await page.ev("[...document.querySelectorAll('details')].forEach(d => d.open = true)");
    const memberCard = await page.text();
    check('member: the card names the key, the group key and the registry, and shows the challenge',
      memberCard.includes(publicKeyHex.slice(0, 8)) && memberCard.includes(group.slice(0, 8)) &&
      memberCard.includes('127.0.0.1') && memberCard.includes(expected.challenge));
    await page.ev('window.__slider.__confirm()', true);
    const proof = await member;
    check('member: answered as {t:"result", id, assertion, origin}', answerShape(proof, 'm1', 'assertion'));
    check('member: the assertion is over the challenge the page computed (= the registry\'s)',
      clientData(proof.assertion).challenge === expected.challengeBase64url && clientData(proof.assertion).type === 'webauthn.get');
    check('member: signed by the key just created, user verified',
      proof.assertion.credentialId === credentialId && await verifies(proof.assertion, key) && userVerified(proof.assertion));
    check('member: `n` goes up across the session', typeof answer.n === 'number' && proof.n > answer.n, `${answer.n} → ${proof.n}`);
    const memberOp = { type: 'sign_member_proof', credential_id: credHex(credentialId), public_key_hex: publicKeyHex,
      attestation_hex: '', transports: '', method: 'clear_signer', group_public_key_hex: group };
    const judgedMember = judge(memberOp, proof, unb64url(expected.challengeBase64url));
    check('core: vela-core accepts the member proof over the challenge the WALLET fetched', !!judgedMember.assertion,
      JSON.stringify(judgedMember).slice(0, 140));
    const strayMember = judge(memberOp, proof, new Uint8Array(32).fill(7));
    check('core: …and refuses it against any other challenge', strayMember.refused && strayMember.refused.code === 'wrong_challenge',
      JSON.stringify(strayMember));

    await page.waitFor("window.__velaState.phase === 'waiting'");
    wallet.bye();
    check('session: bye ends it, and the page says it is done',
      await page.waitFor("window.__velaState.phase === 'ended' && window.__velaState.endReason === 'bye'") &&
      /Done/.test(await page.status()));
    await wallet.stop();
  }

  // B2. sign-in → verify → recover ×2, one session, ended by a close without bye
  const recoveries = [];
  {
    const wallet = loopbackWallet({ token: 'tok-signin' });
    await wallet.listening;
    await page.navigate(`${SIGNER}?ch=ws&lang=en&s=2#p=${wallet.port}&t=tok-signin`);
    await wallet.hello;

    const signIn = wallet.request('s1', { method: 'vela_signIn', params: [{}], origin: '' }, {});
    await page.waitFor("window.__velaState.phase === 'card' && window.__velaState.kind === 'signIn'");
    await page.ev("[...document.querySelectorAll('details')].forEach(d => d.open = true)");
    const shownChallenge = await page.ev("document.querySelector('.challenge-text') && document.querySelector('.challenge-text').textContent");
    check('sign-in: its own card, with the challenge the page made shown in full',
      /Sign in/.test(await page.text()) && /^vela-signin-\d{13}-[0-9a-f]{16}$/.test(shownChallenge || ''), shownChallenge);
    await page.ev('window.__slider.__confirm()', true);
    const answer = await signIn;
    check('sign-in: answered as {t:"result", id, assertion, origin}', answerShape(answer, 's1', 'assertion'));
    check('sign-in: it signed exactly the string on the card', challengeText(answer.assertion) === shownChallenge);
    check('sign-in: the key found is the one created, and it verifies', answer.assertion.credentialId === credentialId &&
      await verifies(answer.assertion, key));
    const handle = answer.assertion.userHandle ? text(answer.assertion.userHandle).split('\u0000') : [];
    check('sign-in: the user handle is "name\\0uuid", so the wallet reads its name back',
      handle[0] === WALLET && UUID_V4.test(handle[1] || ''), JSON.stringify(handle));
    const judgedSignIn = judge({ type: 'authenticate_passkey', method: 'clear_signer' }, answer);
    check('core: vela-core accepts the sign-in, found behind https://getvela.app',
      !!judgedSignIn.assertion && judgedSignIn.assertion.signer_origin === 'https://getvela.app' &&
      judgedSignIn.assertion.credential_id === credHex(credentialId), JSON.stringify(judgedSignIn).slice(0, 140));
    const asProof = judge({ type: 'sign_proof', credential_id: credHex(credentialId), transports: '', method: 'clear_signer', purpose: 'verify' }, answer);
    check('core: a sign-in answer is not a proof — vela-core refuses it by its challenge',
      asProof.refused && asProof.refused.code === 'wrong_challenge', JSON.stringify(asProof));

    // Each further request waits for its own card, then the person slides.
    async function approve(id, intent, kind) {
      await page.waitFor("window.__velaState.phase === 'waiting'");
      const pending = wallet.request(id, intent, {});
      await page.waitFor(`window.__velaState.phase === 'card' && window.__velaState.kind === '${kind}'`);
      const card = await page.text();
      await page.ev('window.__slider.__confirm()', true);
      return { answer: await pending, card };
    }

    const verify = await approve('v1', { method: 'vela_proof', params: [{ credentialId, purpose: 'verify' }], origin: '' }, 'proof');
    check('proof: verify has its own card and signs "vela-verify-<ms>" with the named key',
      /Confirm your key/.test(verify.card) && answerShape(verify.answer, 'v1', 'assertion') &&
      /^vela-verify-\d{13}$/.test(challengeText(verify.answer.assertion)) &&
      verify.answer.assertion.credentialId === credentialId && await verifies(verify.answer.assertion, key),
      challengeText(verify.answer.assertion));
    const judgedVerify = judge({ type: 'sign_proof', credential_id: credHex(credentialId), transports: '', method: 'clear_signer', purpose: 'verify' }, verify.answer);
    check('core: vela-core accepts the verify proof for the named key', !!judgedVerify.assertion, JSON.stringify(judgedVerify).slice(0, 140));

    const first = await approve('r1', { method: 'vela_proof', params: [{ purpose: 'recover_first' }], origin: '' }, 'proof');
    const second = await approve('r2', { method: 'vela_proof', params: [{ credentialId, purpose: 'recover_second' }], origin: '' }, 'proof');
    check('proof: recovery 1 of 2 and 2 of 2 each say which step, and sign "vela-recover-<ms>"',
      /1 of 2/.test(first.card) && /2 of 2/.test(second.card) &&
      /^vela-recover-\d{13}$/.test(challengeText(first.answer.assertion)) &&
      /^vela-recover-\d{13}$/.test(challengeText(second.answer.assertion)));
    recoveries.push(first.answer.assertion, second.answer.assertion);
    const recovered = core.recoverPublicKeyFromAssertions(
      fromHex(first.answer.assertion.authenticatorData), fromHex(first.answer.assertion.clientDataJSON),
      fromHex(first.answer.assertion.signatureDer),
      fromHex(second.answer.assertion.authenticatorData), fromHex(second.answer.assertion.clientDataJSON),
      fromHex(second.answer.assertion.signatureDer),
    );
    check('proof: vela-core recovers the created key from the two recovery signatures',
      !!recovered && recovered.x === key.x && recovered.y === key.y);
    const ns1 = [answer.n, verify.answer.n, first.answer.n, second.answer.n];
    check('session: four requests on one socket, each answered in turn, `n` rising',
      (await page.ev('window.__velaState.received')) === 4 && ns1.every((n, i) => i === 0 || n > ns1[i - 1]), ns1.join(' → '));

    await page.waitFor("window.__velaState.phase === 'waiting'");
    wallet.drop();
    check('session: a close without bye ends it too, as done',
      await page.waitFor("window.__velaState.phase === 'ended' && window.__velaState.endReason === 'closed'") &&
      /Done/.test(await page.status()));
    await wallet.stop();
  }

  // B3. nothing more arrives: the session ends itself, and says so
  {
    const idle = await Page.open(CDP, 'about:blank');
    await idle.send('Page.enable');
    await idle.send('Page.addScriptToEvaluateOnNewDocument', { source: 'window.__velaIdleMs = 1500;' });
    const wallet = loopbackWallet({ token: 'tok-idle' });
    await wallet.listening;
    await idle.navigate(`${SIGNER}?ch=ws&lang=en#p=${wallet.port}&t=tok-idle`);
    await wallet.hello;
    // One request first (refused at once: its challenge was supplied), so
    // the idle clock is the one that runs AFTER an answer.
    const refused = await wallet.request('x1', { method: 'vela_signIn', params: [{ challenge: 'AAAA' }], origin: '' }, {});
    const bye = await wallet.expect((m) => m.t === 'bye', 8000).catch(() => null);
    check('idle: after an answer, a quiet session ends with {t:"bye", reason:"idle"}',
      refused.t === 'error' && !!bye && bye.reason === 'idle');
    check('idle: and the page says it stopped waiting',
      await idle.waitFor("window.__velaState.endReason === 'idle'") && /stopped waiting/.test(await idle.status()));
    await wallet.stop();
    await idle.close();
  }

  // === C. postMessage, from a getvela.app web wallet =========================
  {
    const opener = await Page.open(CDP, 'https://getvela.app/samples/wallet-sim.html');
    await opener.waitFor('!!window.__open');
    await opener.ev('window.__open().then(() => true)', true);
    const popup = await Page.find(CDP, (u) => u.includes('sign.html?ch=post'));
    check('post: the wallet opened the page and it said ready', !!popup);
    if (popup) {
      await popup.addAuthenticator();
      await opener.ev(`window.__p1 = window.__request('p1', { method: 'vela_createPasskey', params: [{ name: 'Web Wallet' }], origin: '' }, { walletName: 'Web Wallet' }); true`);
      await popup.waitFor("window.__velaState && window.__velaState.phase === 'card' && window.__velaState.kind === 'create'");
      const card = await popup.text();
      check('post: a create from https://getvela.app is let through, and the card names the site',
        card.includes('Create a key') && card.includes('getvela.app') && !(await popup.ev('window.__refused')));
      await popup.ev('window.__slider.__confirm()', true);
      await opener.waitFor('window.__answers.length === 1', 15000);
      const made = await opener.ev('window.__answers[0]');
      check('post: answered as {vela:"result", id, registration, origin}',
        made.vela === 'result' && made.id === 'p1' && !!made.registration && isBareHex(made.registration.attestationObject) &&
        made.origin === 'https://getvela.app');
      const webKey = core.extractAttestationPublicKey(fromHex(made.registration.attestationObject));
      check('post: the page stays open and waits for the next request',
        await popup.waitFor("window.__velaState.phase === 'waiting'"));

      await opener.ev(`window.__p2 = window.__request('p2', { method: 'vela_signIn', params: [{}], origin: '' }, {}); true`);
      await popup.waitFor("window.__velaState.phase === 'card' && window.__velaState.kind === 'signIn'");
      await popup.ev('window.__slider.__confirm()', true);
      await opener.waitFor('window.__answers.length === 2', 15000);
      const signedIn = await opener.ev('window.__answers[1]');
      check('post: the second request, on the same page, signs in with the key made by the first',
        signedIn.vela === 'result' && signedIn.id === 'p2' && signedIn.assertion &&
        signedIn.assertion.credentialId === made.registration.credentialId &&
        /^vela-signin-/.test(challengeText(signedIn.assertion)) && await verifies(signedIn.assertion, webKey));
      await popup.waitFor("window.__velaState.phase === 'waiting'");
      await opener.ev('window.__bye()');
      check('post: bye ends the session',
        await popup.waitFor("window.__velaState.phase === 'ended' && window.__velaState.endReason === 'bye'"));
      await popup.close();
    }
    await opener.close();
  }

  const errors = page.console.filter((line) => !/favicon|ERR_NAME_NOT_RESOLVED/.test(line));
  check('the page threw nothing along the way', errors.length === 0, errors.slice(0, 2).join(' | '));
} catch (error) {
  console.log('FAILED: ' + (error.stack || error.message));
  check.results.push(false);
} finally {
  browser.kill();
  await registry.close();
  process.exit(check.summary() ? 0 : 1);
}
