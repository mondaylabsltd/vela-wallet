/**
 * NFT API — 查询钱包在多个网络上持有的 NFT。
 *
 * 使用 Alchemy NFT API v3，并行查询所有支持的网络后聚合返回。
 */
import { env } from '$env/dynamic/private';
import type { RequestHandler } from './$types';

const ALCHEMY_API_KEY = env.ALCHEMY_API_KEY ?? '';

const NETWORK_NAMES: Record<string, string> = {
	'eth-mainnet': 'Ethereum',
	'arb-mainnet': 'Arbitrum',
	'base-mainnet': 'Base',
	'opt-mainnet': 'Optimism',
	'matic-mainnet': 'Polygon',
	'bnb-mainnet': 'BNB Chain',
	'avax-mainnet': 'Avalanche'
};

const ALCHEMY_SLUGS: Record<string, string> = {
	'eth-mainnet': 'eth-mainnet',
	'arb-mainnet': 'arb-mainnet',
	'base-mainnet': 'base-mainnet',
	'opt-mainnet': 'opt-mainnet',
	'matic-mainnet': 'polygon-mainnet',
	'bnb-mainnet': 'bnb-mainnet',
	'avax-mainnet': 'avax-mainnet'
};

const SUPPORTED_NETWORKS = Object.keys(NETWORK_NAMES);

interface NftItem {
	network: string;
	chainName: string;
	contractAddress: string;
	tokenId: string;
	name: string | null;
	description: string | null;
	image: string | null;
	tokenType: string;
	collectionName: string | null;
	collectionImage: string | null;
}

async function fetchNftsForNetwork(
	network: string,
	owner: string,
	pageSize: number,
	pageKey?: string
): Promise<{ nfts: NftItem[]; pageKey: string | null }> {
	const slug = ALCHEMY_SLUGS[network];
	if (!slug) return { nfts: [], pageKey: null };

	const params = new URLSearchParams({
		owner,
		withMetadata: 'true',
		pageSize: String(pageSize)
	});
	if (pageKey) params.set('pageKey', pageKey);

	const url = `https://${slug}.g.alchemy.com/nft/v3/${ALCHEMY_API_KEY}/getNFTsForOwner?${params}`;

	const res = await fetch(url);
	if (!res.ok) {
		console.error(`[nft] ${network} error: ${res.status}`);
		return { nfts: [], pageKey: null };
	}

	const data = await res.json();
	const chainName = NETWORK_NAMES[network] ?? network;

	const nfts: NftItem[] = (data.ownedNfts ?? []).map((nft: Record<string, unknown>) => {
		const contract = nft.contract as Record<string, unknown> | undefined;
		const image = nft.image as Record<string, unknown> | undefined;

		// 解析图片 URL：优先 cachedUrl → pngUrl → originalUrl → thumbnailUrl
		const imageUrl =
			(image?.cachedUrl as string) ||
			(image?.pngUrl as string) ||
			(image?.originalUrl as string) ||
			(image?.thumbnailUrl as string) ||
			null;

		return {
			network,
			chainName,
			contractAddress: (contract?.address as string) ?? '',
			tokenId: (nft.tokenId as string) ?? '',
			name: (nft.name as string) || (contract?.name as string) || null,
			description: (nft.description as string) || null,
			image: imageUrl,
			tokenType: (contract?.tokenType as string) ?? 'UNKNOWN',
			collectionName: (contract?.name as string) || (contract?.openSeaMetadata as Record<string, unknown>)?.collectionName as string || null,
			collectionImage: (contract?.openSeaMetadata as Record<string, unknown>)?.imageUrl as string || null
		};
	});

	return {
		nfts,
		pageKey: (data.pageKey as string) || null
	};
}

export const GET: RequestHandler = async ({ url }) => {
	const address = url.searchParams.get('address');
	const network = url.searchParams.get('network'); // 可选：指定单个网络
	const pageSize = Math.min(parseInt(url.searchParams.get('pageSize') ?? '50'), 100);
	const pageKey = url.searchParams.get('pageKey') ?? undefined;

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
		// 如果指定了网络，只查单个网络（支持分页）
		if (network) {
			if (!NETWORK_NAMES[network]) {
				return new Response(JSON.stringify({ error: `Unsupported network: ${network}` }), {
					status: 400,
					headers: { 'Content-Type': 'application/json' }
				});
			}

			const result = await fetchNftsForNetwork(network, address, pageSize, pageKey);
			return new Response(JSON.stringify({
				nfts: result.nfts,
				pageKey: result.pageKey,
				totalByNetwork: { [network]: result.nfts.length }
			}), {
				headers: {
					'Content-Type': 'application/json',
					'Cache-Control': 'public, max-age=30'
				}
			});
		}

		// 并行查询所有网络（首页）
		const results = await Promise.all(
			SUPPORTED_NETWORKS.map(async (net) => {
				const result = await fetchNftsForNetwork(net, address, pageSize);
				return { network: net, ...result };
			})
		);

		const allNfts: NftItem[] = [];
		const totalByNetwork: Record<string, number> = {};
		const pageKeys: Record<string, string> = {};

		for (const result of results) {
			allNfts.push(...result.nfts);
			totalByNetwork[result.network] = result.nfts.length;
			if (result.pageKey) {
				pageKeys[result.network] = result.pageKey;
			}
		}

		return new Response(JSON.stringify({
			nfts: allNfts,
			totalByNetwork,
			pageKeys: Object.keys(pageKeys).length > 0 ? pageKeys : undefined
		}), {
			headers: {
				'Content-Type': 'application/json',
				'Cache-Control': 'public, max-age=30'
			}
		});
	} catch (err) {
		console.error('[nft] Failed:', err);
		return new Response(JSON.stringify({ error: 'Failed to fetch NFTs' }), {
			status: 502,
			headers: { 'Content-Type': 'application/json' }
		});
	}
};
