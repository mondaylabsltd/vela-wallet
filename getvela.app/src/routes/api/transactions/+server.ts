import { env } from '$env/dynamic/private';
import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

const ALCHEMY_API_KEY = env.ALCHEMY_API_KEY ?? '';

/** Network display names */
const NETWORK_NAMES: Record<string, string> = {
	'eth-mainnet': 'Ethereum',
	'arb-mainnet': 'Arbitrum',
	'base-mainnet': 'Base',
	'opt-mainnet': 'Optimism',
	'matic-mainnet': 'Polygon',
	'bnb-mainnet': 'BNB Chain',
	'avax-mainnet': 'Avalanche',
};

/** Native token symbols per network */
const NATIVE_SYMBOLS: Record<string, string> = {
	'eth-mainnet': 'ETH', 'arb-mainnet': 'ETH', 'base-mainnet': 'ETH',
	'opt-mainnet': 'ETH', 'matic-mainnet': 'POL', 'bnb-mainnet': 'BNB',
	'avax-mainnet': 'AVAX',
};

const SUPPORTED_NETWORKS = Object.keys(NETWORK_NAMES);

/**
 * GET /api/transactions?address=0x...&network=eth-mainnet&pageSize=25&pageKey=...
 *
 * Returns normalized transaction history. If no network specified, queries all networks.
 * Response format is provider-agnostic — can swap Alchemy for Moralis/Covalent later.
 */
export const GET: RequestHandler = async ({ url }) => {
	const address = url.searchParams.get('address');
	if (!address) return json({ error: 'Missing address parameter' }, { status: 400 });

	const network = url.searchParams.get('network');
	const pageSize = Math.min(parseInt(url.searchParams.get('pageSize') || '25'), 100);
	const pageKey = url.searchParams.get('pageKey') || undefined;

	const networks = network ? [network] : SUPPORTED_NETWORKS;

	try {
		const allTxs: NormalizedTransaction[] = [];

		// Query each network in parallel
		const results = await Promise.allSettled(
			networks.map((net) => fetchTransactions(address, net, pageSize, pageKey))
		);

		for (const result of results) {
			if (result.status === 'fulfilled') {
				allTxs.push(...result.value.transactions);
			}
		}

		// Sort by timestamp descending (newest first)
		allTxs.sort((a, b) => (b.timestamp || 0) - (a.timestamp || 0));

		// Limit to pageSize
		const limited = allTxs.slice(0, pageSize);

		return json({
			transactions: limited,
			totalCount: allTxs.length,
		});
	} catch (e) {
		return json({ error: (e as Error).message }, { status: 500 });
	}
};

// ─── Normalized Transaction Type (provider-agnostic) ───

interface NormalizedTransaction {
	hash: string;
	network: string;
	chainName: string;
	from: string;
	to: string;
	value: string;         // decimal string (e.g. "0.05")
	symbol: string;        // "ETH", "USDC", etc.
	decimals: number;
	tokenAddress: string | null;  // null = native transfer
	category: 'send' | 'receive' | 'contract' | 'approve';
	timestamp: number | null;     // unix seconds
	blockNumber: string;
	status: 'confirmed' | 'failed';
	/** For ERC-721/1155: token ID */
	tokenId: string | null;
	/** Raw data field for contract interactions */
	data: string | null;
}

// ─── Alchemy Provider Implementation ───

interface AlchemyTransferResult {
	transfers: AlchemyTransfer[];
	pageKey?: string;
}

interface AlchemyTransfer {
	hash: string;
	from: string;
	to: string;
	value: number | null;
	asset: string | null;
	category: string;
	blockNum: string;
	metadata: { blockTimestamp?: string };
	rawContract: {
		value: string | null;
		address: string | null;
		decimal: string | null;
	};
	tokenId: string | null;
}

async function fetchTransactions(
	address: string,
	network: string,
	pageSize: number,
	pageKey?: string,
): Promise<{ transactions: NormalizedTransaction[]; pageKey?: string }> {
	const rpcUrl = `https://${network}.g.alchemy.com/v2/${ALCHEMY_API_KEY}`;

	// Fetch both sent and received transfers
	const [sentResult, receivedResult] = await Promise.all([
		alchemyGetTransfers(rpcUrl, { fromAddress: address }, pageSize, pageKey),
		alchemyGetTransfers(rpcUrl, { toAddress: address }, pageSize, pageKey),
	]);

	const seen = new Set<string>();
	const transactions: NormalizedTransaction[] = [];

	for (const transfer of [...sentResult.transfers, ...receivedResult.transfers]) {
		// Dedup by hash + category (same tx can appear in both sent/received)
		const key = `${transfer.hash}_${transfer.category}_${transfer.rawContract?.address || 'native'}`;
		if (seen.has(key)) continue;
		seen.add(key);

		const normalized = normalizeTransfer(transfer, address, network);
		if (normalized) transactions.push(normalized);
	}

	return { transactions, pageKey: sentResult.pageKey || receivedResult.pageKey };
}

async function alchemyGetTransfers(
	rpcUrl: string,
	filter: { fromAddress?: string; toAddress?: string },
	maxCount: number,
	pageKey?: string,
): Promise<AlchemyTransferResult> {
	const body = {
		id: 1,
		jsonrpc: '2.0',
		method: 'alchemy_getAssetTransfers',
		params: [{
			...filter,
			category: ['external', 'internal', 'erc20', 'erc721', 'erc1155'],
			order: 'desc',
			maxCount: `0x${maxCount.toString(16)}`,
			withMetadata: true,
			...(pageKey ? { pageKey } : {}),
		}],
	};

	const resp = await fetch(rpcUrl, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body),
	});

	const json = await resp.json();
	if (json.error) throw new Error(json.error.message);
	return json.result as AlchemyTransferResult;
}

function normalizeTransfer(
	transfer: AlchemyTransfer,
	userAddress: string,
	network: string,
): NormalizedTransaction | null {
	const isFromUser = transfer.from.toLowerCase() === userAddress.toLowerCase();
	const isToUser = transfer.to?.toLowerCase() === userAddress.toLowerCase();

	let category: NormalizedTransaction['category'];
	if (transfer.category === 'erc20' && !transfer.value && transfer.rawContract?.value === '0x') {
		category = 'approve';
	} else if (isFromUser && isToUser) {
		category = 'contract'; // self-transfer or contract interaction
	} else if (isFromUser) {
		category = 'send';
	} else {
		category = 'receive';
	}

	// Parse value
	let value = '0';
	let symbol = transfer.asset || NATIVE_SYMBOLS[network] || 'ETH';
	let decimals = 18;
	let tokenAddress: string | null = null;

	if (transfer.category === 'external' || transfer.category === 'internal') {
		// Native transfer
		value = transfer.value?.toString() || '0';
		symbol = NATIVE_SYMBOLS[network] || 'ETH';
	} else if (transfer.category === 'erc20') {
		value = transfer.value?.toString() || '0';
		tokenAddress = transfer.rawContract?.address || null;
		decimals = parseInt(transfer.rawContract?.decimal || '18');
	} else if (transfer.category === 'erc721' || transfer.category === 'erc1155') {
		value = '1';
		tokenAddress = transfer.rawContract?.address || null;
		decimals = 0;
	}

	// Parse timestamp
	let timestamp: number | null = null;
	if (transfer.metadata?.blockTimestamp) {
		timestamp = Math.floor(new Date(transfer.metadata.blockTimestamp).getTime() / 1000);
	}

	return {
		hash: transfer.hash,
		network,
		chainName: NETWORK_NAMES[network] || network,
		from: transfer.from,
		to: transfer.to || '',
		value,
		symbol,
		decimals,
		tokenAddress,
		category,
		timestamp,
		blockNumber: transfer.blockNum,
		status: 'confirmed',
		tokenId: transfer.tokenId,
		data: null,
	};
}
