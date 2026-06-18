<script lang="ts">
	import { page } from '$app/state';
	import Seo from '$lib/components/Seo.svelte';

	const WALLET_BASE = 'https://wallet.getvela.app';
	const ONBOARDING = `${WALLET_BASE}/onboarding`;

	// --- read request params -------------------------------------------------
	const p = $derived(page.url.searchParams);
	const to = $derived((p.get('to') ?? '').trim());
	const chainId = $derived(parseInt(p.get('chain') ?? '', 10));
	const token = $derived((p.get('token') ?? '').trim()); // empty = native coin
	const amount = $derived((p.get('amount') ?? '').trim()); // human-readable, optional
	const symbol = $derived((p.get('sym') ?? '').trim() || 'tokens');
	const decimals = $derived(parseInt(p.get('dec') ?? '18', 10) || 18);
	const networkName = $derived((p.get('net') ?? '').trim() || (Number.isFinite(chainId) ? `Chain ${chainId}` : ''));

	const valid = $derived(/^0x[0-9a-fA-F]{40}$/.test(to) && Number.isFinite(chainId));
	const isNative = $derived(!token);

	// --- derived links -------------------------------------------------------
	function toBaseUnits(a: string, dec: number): string {
		if (!a) return '';
		const [i, f = ''] = a.split('.');
		const frac = (f + '0'.repeat(dec)).slice(0, dec);
		const digits = ((i || '0') + frac).replace(/^0+(?=\d)/, '');
		return digits === '' ? '0' : digits;
	}
	const amountBase = $derived(amount ? toBaseUnits(amount, decimals) : '');

	// EIP-681 URI for other wallets.
	const eip681 = $derived(
		isNative
			? `ethereum:${to}@${chainId}${amountBase ? `?value=${amountBase}` : ''}`
			: `ethereum:${token}@${chainId}/transfer?address=${to}${amountBase ? `&uint256=${amountBase}` : ''}`
	);

	// Deep link into the Vela web wallet's Send screen — locked & pre-filled.
	const velaLink = $derived(
		`${WALLET_BASE}/send?prefilledRecipient=${to}&prefilledChainId=${chainId}` +
			(token ? `&prefilledTokenAddress=${token}` : '') +
			(amountBase ? `&prefilledAmountBase=${amountBase}` : '') +
			`&locked=1`
	);

	const headline = $derived(amount ? `Request ${amount} ${symbol}` : `Request ${symbol}`);
	const shortTo = $derived(to ? `${to.slice(0, 10)}…${to.slice(-8)}` : '');

	// --- copy + QR -----------------------------------------------------------
	let copiedKey = $state<string | null>(null);
	async function copy(key: string, value: string) {
		try {
			await navigator.clipboard.writeText(value);
			copiedKey = key;
			setTimeout(() => { if (copiedKey === key) copiedKey = null; }, 1500);
		} catch {}
	}

	let showManual = $state(false);
	let qrDataUrl = $state('');

	// Render a QR of the EIP-681 URI client-side (loads the qrcode lib from CDN,
	// so the marketing build needs no extra dependency). Best-effort: links still
	// work if it fails to load.
	$effect(() => {
		const uri = eip681;
		if (!valid || typeof window === 'undefined') return;
		let cancelled = false;
		const render = async () => {
			try {
				const w = window as unknown as { QRCode?: { toDataURL: (t: string, o: unknown, cb: (e: unknown, url: string) => void) => void } };
				if (!w.QRCode) {
					await new Promise<void>((res, rej) => {
						const s = document.createElement('script');
						s.src = 'https://cdn.jsdelivr.net/npm/qrcode@1.5.4/build/qrcode.min.js';
						s.onload = () => res();
						s.onerror = () => rej(new Error('qr load failed'));
						document.head.appendChild(s);
					});
				}
				w.QRCode?.toDataURL(uri, { margin: 1, width: 320, color: { dark: '#0f0e0c', light: '#ffffff' } }, (err, url) => {
					if (!err && !cancelled) qrDataUrl = url;
				});
			} catch {}
		};
		render();
		return () => { cancelled = true; };
	});
</script>

<Seo
	title="Payment request"
	description="Pay a Vela Wallet request in one tap, with any wallet, or by hand."
	canonical="/pay"
	noindex
/>

<nav>
	<a class="logo" href="/">
		<img src="/vela-logo.png" alt="Vela Wallet" width="28" height="28" />
		<span>Vela Wallet</span>
	</a>
</nav>

<main>
	{#if !valid}
		<div class="card error">
			<h1>Invalid payment link</h1>
			<p>This link is missing a valid recipient or network. Ask the sender for a new one.</p>
			<a class="btn ghost" href="/">Go to Vela Wallet</a>
		</div>
	{:else}
		<div class="card">
			<p class="eyebrow">Payment request</p>
			<h1>{headline}</h1>
			<p class="sub">on {networkName}</p>

			<button class="addr" onclick={() => copy('to', to)} title="Copy address">
				<span>{shortTo}</span>
				<span class="copy">{copiedKey === 'to' ? 'Copied' : 'Copy'}</span>
			</button>

			<a class="btn primary" href={velaLink}>Open in Vela Wallet</a>

			<button class="btn ghost" onclick={() => (showManual = !showManual)}>
				Pay with another wallet
			</button>

			{#if showManual}
				<div class="manual">
					{#if qrDataUrl}
						<div class="qr">
							<img src={qrDataUrl} alt="EIP-681 payment QR" width="180" height="180" />
							<p class="qr-hint">Scan with an EIP-681 wallet</p>
						</div>
					{/if}

					<a class="btn outline" href={eip681}>Open in wallet app</a>

					<p class="manual-note">
						Wallet doesn't support payment links? Enter these details by hand:
					</p>

					<dl class="details">
						<div>
							<dt>Recipient</dt>
							<dd>
								<button onclick={() => copy('m-to', to)}>
									<code>{shortTo}</code><span>{copiedKey === 'm-to' ? '✓' : 'Copy'}</span>
								</button>
							</dd>
						</div>
						<div>
							<dt>Network</dt>
							<dd><span class="plain">{networkName} <em>(chain {chainId})</em></span></dd>
						</div>
						<div>
							<dt>Token</dt>
							<dd>
								{#if isNative}
									<span class="plain">{symbol} (native coin)</span>
								{:else}
									<button onclick={() => copy('m-token', token)}>
										<code>{symbol} · {token.slice(0, 8)}…{token.slice(-6)}</code><span>{copiedKey === 'm-token' ? '✓' : 'Copy'}</span>
									</button>
								{/if}
							</dd>
						</div>
						<div>
							<dt>Amount</dt>
							<dd><span class="plain">{amount ? `${amount} ${symbol}` : 'Any amount'}</span></dd>
						</div>
					</dl>
				</div>
			{/if}
		</div>

		<p class="foot">
			New to Vela? <a href={ONBOARDING}>Create a wallet</a> — no seed phrase, just your fingerprint.
		</p>
	{/if}
</main>

<style>
	nav {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--header-h, 60px);
		border-bottom: 1px solid var(--border);
	}
	.logo {
		display: inline-flex;
		align-items: center;
		gap: 0.55rem;
		color: var(--text);
		text-decoration: none;
		font-weight: 600;
	}
	main {
		max-width: 460px;
		margin: 0 auto;
		padding: 2.5rem 1.25rem 4rem;
	}
	.card {
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 2rem 1.5rem;
		text-align: center;
	}
	.eyebrow {
		font-size: 0.8rem;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--text-muted);
		margin-bottom: 0.75rem;
	}
	h1 {
		font-size: 1.9rem;
		font-weight: 700;
		color: var(--text);
		line-height: 1.15;
	}
	.sub {
		color: var(--accent);
		font-weight: 600;
		margin-top: 0.35rem;
	}
	.addr {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		width: 100%;
		margin: 1.5rem 0;
		padding: 0.75rem 1rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
		font-size: 0.95rem;
		color: var(--text);
		cursor: pointer;
	}
	.addr .copy {
		font-family: var(--font-sans);
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.btn {
		display: block;
		width: 100%;
		padding: 0.9rem 1rem;
		border-radius: var(--radius-sm);
		font-weight: 600;
		font-size: 1rem;
		text-align: center;
		text-decoration: none;
		cursor: pointer;
		border: 1px solid transparent;
		margin-top: 0.75rem;
		font-family: inherit;
	}
	.btn.primary {
		background: var(--accent);
		color: #fff;
	}
	.btn.primary:hover { background: var(--accent-hover); }
	.btn.ghost {
		background: transparent;
		color: var(--text-secondary);
		border-color: var(--border);
	}
	.btn.outline {
		background: var(--bg-raised);
		color: var(--text);
		border-color: var(--border-strong);
	}
	.manual {
		margin-top: 1.25rem;
		padding-top: 1.25rem;
		border-top: 1px solid var(--border);
		text-align: left;
	}
	.qr {
		text-align: center;
		margin-bottom: 1rem;
	}
	.qr img {
		border-radius: var(--radius-sm);
		background: #fff;
		padding: 8px;
	}
	.qr-hint, .manual-note {
		font-size: 0.85rem;
		color: var(--text-muted);
		text-align: center;
		margin-top: 0.5rem;
	}
	.manual-note { margin: 1.25rem 0 0.75rem; }
	.details {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.details > div {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.6rem 0.75rem;
		background: var(--bg-raised);
		border-radius: var(--radius-sm);
	}
	dt {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	dd { margin: 0; }
	.details button {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		background: none;
		border: none;
		color: var(--text);
		cursor: pointer;
		font-family: inherit;
	}
	.details code {
		font-family: var(--font-mono);
		font-size: 0.85rem;
		color: var(--text);
	}
	.details button span {
		font-size: 0.75rem;
		color: var(--link);
	}
	.plain {
		font-size: 0.9rem;
		color: var(--text);
	}
	.plain em { color: var(--text-muted); font-style: normal; }
	.foot {
		text-align: center;
		color: var(--text-muted);
		font-size: 0.9rem;
		margin-top: 1.5rem;
	}
	.foot a { color: var(--link); }
	.error h1 { font-size: 1.3rem; }
	.error p { color: var(--text-secondary); margin: 0.75rem 0 1.25rem; }
</style>
