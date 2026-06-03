import satori from 'satori';
import { initWasm, Resvg } from '@resvg/resvg-wasm';
import { getTemplate } from '@shelchin/seo-sveltekit/og/templates';
import type { OgImageParams } from '@shelchin/seo-sveltekit';
import type { RequestHandler } from '@sveltejs/kit';
import { seoConfig } from '$lib/seo';

// WASM initialization state
let wasmInitialized = false;
let wasmInitPromise: Promise<void> | null = null;

// Default fonts from Google Fonts CDN (TTF format for satori compatibility)
const DEFAULT_FONTS = [
	{
		name: 'Inter',
		weight: 400 as const,
		style: 'normal' as const,
		source: 'https://fonts.gstatic.com/s/inter/v18/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuLyfMZg.ttf',
	},
	{
		name: 'Inter',
		weight: 700 as const,
		style: 'normal' as const,
		source: 'https://fonts.gstatic.com/s/inter/v18/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuFuYMZg.ttf',
	},
];

// Font cache
const fontCache = new Map<string, ArrayBuffer>();

/**
 * Initialize WASM module (only once)
 */
async function ensureWasmInitialized(): Promise<void> {
	if (wasmInitialized) return;

	if (!wasmInitPromise) {
		wasmInitPromise = (async () => {
			try {
				// Fetch WASM from CDN (works in both dev and binary)
				const wasmUrl = 'https://cdn.jsdelivr.net/npm/@resvg/resvg-wasm@2.6.2/index_bg.wasm';
				const response = await fetch(wasmUrl);
				const wasmBuffer = await response.arrayBuffer();
				await initWasm(wasmBuffer);
				wasmInitialized = true;
				console.log('[OG] WASM initialized successfully');
			} catch (error) {
				console.error('[OG] Failed to initialize WASM:', error);
				wasmInitPromise = null;
				throw error;
			}
		})();
	}

	await wasmInitPromise;
}

/**
 * Load font from URL with caching
 */
async function loadFont(
	config: (typeof DEFAULT_FONTS)[0],
	retries = 3
): Promise<ArrayBuffer | null> {
	const cacheKey = `${config.name}-${config.weight}-${config.style}`;
	if (fontCache.has(cacheKey)) {
		return fontCache.get(cacheKey)!;
	}

	for (let i = 0; i < retries; i++) {
		try {
			const response = await fetch(config.source, {
				signal: AbortSignal.timeout(10000),
			});
			if (response.ok) {
				const buffer = await response.arrayBuffer();
				fontCache.set(cacheKey, buffer);
				return buffer;
			}
		} catch (error) {
			console.warn(`[OG] Font fetch attempt ${i + 1}/${retries} failed for ${config.name}:`, error);
			if (i === retries - 1) {
				return null;
			}
			await new Promise((resolve) => setTimeout(resolve, 1000));
		}
	}
	return null;
}

/**
 * Load all fonts
 */
async function loadFonts(): Promise<
	Array<{
		name: string;
		data: ArrayBuffer;
		weight: number;
		style: 'normal' | 'italic';
	}>
> {
	const results = await Promise.all(
		DEFAULT_FONTS.map(async (config) => {
			const data = await loadFont(config);
			if (!data) return null;
			return {
				name: config.name,
				data,
				weight: config.weight,
				style: config.style,
			};
		})
	);
	return results.filter((f): f is NonNullable<typeof f> => f !== null);
}

// Config
const config = {
	siteName: seoConfig.siteName,
	domain: seoConfig.domain,
	defaultTemplate: 'website' as const,
	cacheControl: 's-maxage=31536000, stale-while-revalidate',
	width: 1200,
	height: 630,
	defaultGradient: seoConfig.defaultGradient,
};

export const GET: RequestHandler = async ({ url }) => {
	try {
		// Ensure WASM is initialized
		await ensureWasmInitialized();

		// Parse query parameters
		const title = url.searchParams.get('title') || config.siteName;
		const type = (url.searchParams.get('type') as OgImageParams['type']) || config.defaultTemplate;
		const subtitle = url.searchParams.get('subtitle') || undefined;
		const emoji = url.searchParams.get('emoji') || undefined;
		const readTime = url.searchParams.get('readTime') || undefined;
		const difficulty = url.searchParams.get('difficulty') as OgImageParams['difficulty'] || undefined;
		const author = url.searchParams.get('author') || undefined;
		const date = url.searchParams.get('date') || undefined;
		const gradient = url.searchParams.get('gradient') || config.defaultGradient;
		const price = url.searchParams.get('price') || undefined;
		const ratingStr = url.searchParams.get('rating');
		const rating = ratingStr ? parseFloat(ratingStr) : undefined;
		const duration = url.searchParams.get('duration') || undefined;
		const location = url.searchParams.get('location') || undefined;
		const category = url.searchParams.get('category') || undefined;
		const badge = url.searchParams.get('badge') || undefined;

		const params: OgImageParams = {
			type,
			subtitle,
			emoji,
			readTime,
			difficulty,
			author,
			date,
			gradient,
			price,
			rating,
			duration,
			location,
			category,
			badge,
		};

		// Load fonts
		const loadedFonts = await loadFonts();
		if (loadedFonts.length === 0) {
			console.error('[OG] No fonts available');
			return new Response(JSON.stringify({ error: 'No fonts available' }), {
				status: 500,
				headers: { 'Content-Type': 'application/json' },
			});
		}

		// Get template
		const template = getTemplate(type || 'website');
		const element = template({
			title,
			siteName: config.siteName,
			config: { ...config, defaultGradient: gradient },
			params,
		});

		// Generate SVG with satori
		const svg = await satori(element, {
			width: config.width,
			height: config.height,
			fonts: loadedFonts as Parameters<typeof satori>[1]['fonts'],
		});

		// Convert SVG to PNG with resvg-wasm
		const resvg = new Resvg(svg, {
			fitTo: {
				mode: 'width',
				value: config.width,
			},
		});
		const pngData = resvg.render();
		const pngBuffer = pngData.asPng();

		return new Response(pngBuffer.buffer as ArrayBuffer, {
			headers: {
				'Content-Type': 'image/png',
				'Cache-Control': config.cacheControl,
			},
		});
	} catch (error) {
		console.error('[OG] Generation failed:', error);
		return new Response(JSON.stringify({ error: 'Failed to generate OG image' }), {
			status: 500,
			headers: { 'Content-Type': 'application/json' },
		});
	}
};
