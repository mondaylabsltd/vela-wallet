import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';

const ALCHEMY_API_KEY = env.ALCHEMY_API_KEY ?? '';
const ALCHEMY_URL = `https://api.g.alchemy.com/data/v1/${ALCHEMY_API_KEY}/assets/tokens/by-address`;

/** Alchemy 网络标识 → 前端展示名 */
const NETWORK_NAMES: Record<string, string> = {
	'eth-mainnet': 'Ethereum',
	'arb-mainnet': 'Arbitrum',
	'base-mainnet': 'Base',
	'opt-mainnet': 'Optimism',
	'matic-mainnet': 'Polygon',
	'bnb-mainnet': 'BNB Chain',
	'avax-mainnet': 'Avalanche'
};

/** Native token 信息（Alchemy 对 native token 可能不返回 metadata） */
const NATIVE_TOKENS: Record<string, { symbol: string; name: string }> = {
	'eth-mainnet': { symbol: 'ETH', name: 'Ether' },
	'arb-mainnet': { symbol: 'ETH', name: 'Ether' },
	'base-mainnet': { symbol: 'ETH', name: 'Ether' },
	'opt-mainnet': { symbol: 'ETH', name: 'Ether' },
	'matic-mainnet': { symbol: 'POL', name: 'POL' },
	'bnb-mainnet': { symbol: 'BNB', name: 'BNB' },
	'avax-mainnet': { symbol: 'AVAX', name: 'Avalanche' }
};

const SUPPORTED_NETWORKS = Object.keys(NETWORK_NAMES);

/** 已知合法 ERC-20 symbol（大写）— 无报价也不标记为 spam */
const KNOWN_SYMBOLS = new Set([
	'USDT', 'USDC', 'USDC.E', 'DAI', 'BUSD', 'TUSD', 'FRAX', 'LUSD', 'PYUSD', 'USDP', 'GUSD', 'CRVUSD',
	'WETH', 'WBTC', 'STETH', 'WSTETH', 'RETH', 'CBETH', 'EZETH', 'WEETH',
	'LINK', 'UNI', 'AAVE', 'MKR', 'SNX', 'CRV', 'LDO', 'RPL', 'ENS', 'GRT', 'COMP', 'SUSHI',
	'ARB', 'OP', 'MATIC', 'GMX', 'PENDLE', 'RDNT',
	'SHIB', 'PEPE', 'APE', 'DOGE', 'FLOKI',
]);

export const GET: RequestHandler = async ({ url }) => {
	const address = url.searchParams.get('address');

	if (!address || !/^0x[0-9a-fA-F]{40}$/.test(address)) {
		return new Response(JSON.stringify({ error: 'Invalid address' }), {
			status: 400,
			headers: { 'Content-Type': 'application/json' }
		});
	}

	if (!ALCHEMY_API_KEY) {
		return new Response(JSON.stringify({ error: 'API not configured' }), {
			status: 503,
			headers: { 'Content-Type': 'application/json' }
		});
	}

	try {
		const res = await fetch(ALCHEMY_URL, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				addresses: [{ address, networks: SUPPORTED_NETWORKS }],
				withMetadata: true,
				withPrices: true,
				includeNativeTokens: true,
				includeErc20Tokens: true
			})
		});

		if (!res.ok) {
			return new Response(JSON.stringify({ error: `Alchemy error (${res.status})` }), {
				status: 502,
				headers: { 'Content-Type': 'application/json' }
			});
		}

		const data = await res.json();
		const tokens: Array<{
			network: string;
			chainName: string;
			symbol: string;
			balance: string;
			decimals: number;
			logo: string | null;
			name: string;
			tokenAddress: string | null;
			priceUsd: number | null;
			spam: boolean;
		}> = [];

		for (const token of data?.data?.tokens ?? []) {
			const rawBalance = token.tokenBalance;
			if (!rawBalance || rawBalance === '0' || rawBalance === '0x0') continue;

			const decimals = token.tokenMetadata?.decimals ?? 18;
			const balance = formatTokenBalance(rawBalance, decimals);
			if (parseFloat(balance) === 0) continue;

			const isNative = !token.tokenAddress || token.tokenAddress === '0x0000000000000000000000000000000000000000';
			const nativeFallback = NATIVE_TOKENS[token.network];
			const symbol = token.tokenMetadata?.symbol || (isNative ? nativeFallback?.symbol : null) || '???';
			const name = token.tokenMetadata?.name || (isNative ? nativeFallback?.name : null) || '';

			// 价格（USD）
			const priceEntry = token.tokenPrices?.[0];
			const priceUsd = priceEntry?.currency === 'usd' ? parseFloat(priceEntry.value) : null;
			const validPrice = priceUsd != null && !isNaN(priceUsd);

			// 判断垃圾币：非 native、无报价、不在已知白名单
			const spam = !isNative && !validPrice && !KNOWN_SYMBOLS.has(symbol.toUpperCase());

			tokens.push({
				network: token.network,
				chainName: NETWORK_NAMES[token.network] ?? token.network,
				symbol,
				balance,
				decimals,
				logo: token.tokenMetadata?.logo ?? null,
				name,
				tokenAddress: isNative ? null : (token.tokenAddress ?? null),
				priceUsd: validPrice ? priceUsd : null,
				spam
			});
		}

		return new Response(JSON.stringify({ tokens }), {
			headers: {
				'Content-Type': 'application/json',
				'Cache-Control': 'public, max-age=15'
			}
		});
	} catch {
		return new Response(JSON.stringify({ error: 'Failed to fetch balances' }), {
			status: 502,
			headers: { 'Content-Type': 'application/json' }
		});
	}
};

function formatTokenBalance(rawBalance: string, decimals: number): string {
	try {
		const bi = BigInt(rawBalance);
		if (bi === 0n) return '0';
		const divisor = 10n ** BigInt(decimals);
		const whole = bi / divisor;
		const remainder = bi % divisor;
		if (remainder === 0n) return whole.toString();
		const fracStr = remainder.toString().padStart(decimals, '0').replace(/0+$/, '');
		return `${whole}.${fracStr}`;
	} catch {
		return '0';
	}
}
