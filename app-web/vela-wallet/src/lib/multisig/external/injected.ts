// The connect page's signer (ported from sub3's bridge): the Wallet side of WalletPair forwarding each eip155
// function to the browser's own wallet (EIP-1193). Keys, confirmation, signing and broadcasting all stay in that
// wallet; it shows the owner what is signed. Vela asks for no more than a multisig needs: the SafeTx's typed
// data, a transaction (an owner executing), and a network switch. Anything else is refused here.

import type { Eip1193Provider } from './eip6963';
import { chainOf, evm } from '$lib/walletpair/networks/evm';
import { ErrorCode, type Json } from '$lib/walletpair/protocol';
import { WalletError, type Signer } from '$lib/walletpair/wallet';

/** Functions forwarded unchanged. Those with an effect are marked before they are handed over (wallet.ts). */
const FORWARDED = new Set(['eth_signTypedData_v4', 'eth_sendTransaction']);
const EFFECTS = new Set(['eth_sendTransaction']);

export interface InjectedSigner extends Signer {
	provider: Eip1193Provider;
}

function asWalletError(error: unknown): WalletError {
	const e = error as { code?: unknown; message?: unknown };
	const code =
		typeof e?.code === 'number' && Number.isInteger(e.code) ? e.code : ErrorCode.Internal;
	return new WalletError(code, typeof e?.message === 'string' ? e.message : 'The wallet failed');
}

export async function currentChain(provider: Eip1193Provider): Promise<string> {
	return chainOf((await provider.request({ method: 'eth_chainId' })) as string);
}

export function injectedSigner(
	provider: Eip1193Provider,
	accounts: string[],
	chain: string
): InjectedSigner {
	const forward = async (method: string, params: unknown): Promise<Json> => {
		try {
			return ((await provider.request({ method, params })) ?? null) as Json;
		} catch (error) {
			throw asWalletError(error);
		}
	};

	return {
		network: evm,
		provider,
		chain,
		accounts,
		async perform(method, params, context) {
			if (method === 'wallet_switchEthereumChain') {
				await forward(method, params);
				context.setChain(await currentChain(provider));
				return null;
			}
			if (!FORWARDED.has(method))
				throw new WalletError(ErrorCode.Unsupported, `Unsupported: ${method}`);
			if (method === 'eth_signTypedData_v4' || method === 'eth_sendTransaction') {
				// The wallet app must act on the chain the DApp side saw (core §8, evm.md §2).
				if ((await currentChain(provider)) !== context.state.chain) {
					throw new WalletError(ErrorCode.ChainMismatch, 'The wallet is on another chain');
				}
			}
			if (EFFECTS.has(method)) context.beginEffect();
			return forward(method, params);
		}
	};
}
