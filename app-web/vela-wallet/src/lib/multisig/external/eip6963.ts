// Discovering injected wallets (EIP-6963), with window.ethereum (EIP-1193) as the fallback (ported from sub3).

export interface Eip1193Provider {
	request(args: { method: string; params?: unknown }): Promise<unknown>;
	on?(event: string, listener: (...args: unknown[]) => void): void;
	removeListener?(event: string, listener: (...args: unknown[]) => void): void;
}

export interface ProviderDetail {
	info: { uuid: string; name: string; icon: string; rdns: string };
	provider: Eip1193Provider;
}

/** Wallets that announce themselves within `wait` ms; window.ethereum alone if none does. */
export function discoverProviders(wait = 400): Promise<ProviderDetail[]> {
	return new Promise((resolve) => {
		const found = new Map<string, ProviderDetail>();
		const onAnnounce = (event: Event) => {
			const detail = (event as CustomEvent<ProviderDetail>).detail;
			if (detail?.info?.uuid && detail.provider) found.set(detail.info.uuid, detail);
		};
		window.addEventListener('eip6963:announceProvider', onAnnounce);
		window.dispatchEvent(new Event('eip6963:requestProvider'));
		setTimeout(() => {
			window.removeEventListener('eip6963:announceProvider', onAnnounce);
			const list = [...found.values()];
			const legacy = (window as unknown as { ethereum?: Eip1193Provider }).ethereum;
			if (list.length === 0 && legacy) {
				list.push({
					info: { uuid: 'window.ethereum', name: 'Browser wallet', icon: '', rdns: '' },
					provider: legacy
				});
			}
			resolve(list);
		}, wait);
	});
}
