/**
 * Project-owned OG layouts, adapted from @shelchin/seo-sveltekit 0.1.0 (MIT).
 * Keep all eleven public image types compatible. See THIRD-PARTY-NOTICES.md.
 */
export interface OgImageParams {
	type?:
		| 'website'
		| 'article'
		| 'tool'
		| 'faq'
		| 'howto'
		| 'product'
		| 'event'
		| 'recipe'
		| 'video'
		| 'job'
		| 'review';
	subtitle?: string;
	emoji?: string;
	readTime?: string;
	difficulty?: 'Beginner' | 'Intermediate' | 'Advanced';
	gradient?: string;
	author?: string;
	date?: string;
	price?: string;
	rating?: number;
	duration?: string;
	location?: string;
	category?: string;
	badge?: string;
}

type OgChild = OgElement | string | null | OgChild[];
interface OgElement {
	type: 'div';
	key: null;
	props: { style: Record<string, string | number>; children: OgChild };
}
type OgTemplateRenderer = (input: {
	title: string;
	siteName: string;
	config: { domain?: string; defaultGradient?: string };
	params: OgImageParams;
}) => OgElement;

function div(style: OgElement['props']['style'], children: OgChild): OgElement {
	return { type: 'div', key: null, props: { style, children } };
}

const baseContainerStyle = {
	display: 'flex',
	flexDirection: 'column',
	width: '100%',
	height: '100%',
	padding: '60px',
	fontFamily: 'Inter, Noto Sans SC, Noto Sans JP, sans-serif'
};
const websiteTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #667eea 0%, #764ba2 100%)';
	return div(
		{
			...baseContainerStyle,
			background: gradient,
			justifyContent: 'space-between'
		},
		[
			div(
				{
					display: 'flex',
					alignItems: 'center',
					gap: '16px'
				},
				[
					div(
						{
							fontSize: '28px',
							fontWeight: 600,
							color: 'rgba(255, 255, 255, 0.9)'
						},
						siteName
					)
				]
			),
			div(
				{
					display: 'flex',
					flexDirection: 'column',
					gap: '20px'
				},
				[
					div(
						{
							fontSize: '64px',
							fontWeight: 700,
							color: '#ffffff',
							lineHeight: 1.2,
							maxWidth: '900px'
						},
						title
					),
					params.subtitle
						? div(
								{
									fontSize: '28px',
									fontWeight: 400,
									color: 'rgba(255, 255, 255, 0.8)',
									maxWidth: '800px'
								},
								params.subtitle
							)
						: null
				].filter(Boolean)
			),
			div(
				{
					display: 'flex',
					justifyContent: 'flex-end'
				},
				[
					div(
						{
							fontSize: '24px',
							color: 'rgba(255, 255, 255, 0.7)'
						},
						config.domain?.replace(/^https?:\/\//, '') || ''
					)
				]
			)
		]
	);
};
const toolTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #1e3a5f 0%, #0d1b2a 100%)';
	return div(
		{
			...baseContainerStyle,
			background: gradient,
			justifyContent: 'space-between'
		},
		[
			div(
				{
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'space-between'
				},
				[
					div(
						{
							fontSize: '24px',
							fontWeight: 600,
							color: 'rgba(255, 255, 255, 0.8)'
						},
						siteName
					),
					div(
						{
							display: 'flex',
							alignItems: 'center',
							gap: '8px',
							padding: '8px 16px',
							background: 'rgba(16, 185, 129, 0.2)',
							borderRadius: '20px',
							border: '1px solid rgba(16, 185, 129, 0.4)'
						},
						[
							div(
								{
									fontSize: '16px',
									color: '#10b981',
									fontWeight: 500
								},
								'Free Tool'
							)
						]
					)
				]
			),
			div(
				{
					display: 'flex',
					alignItems: 'center',
					gap: '32px'
				},
				[
					params.emoji
						? div(
								{
									fontSize: '96px'
								},
								params.emoji
							)
						: null,
					div(
						{
							display: 'flex',
							flexDirection: 'column',
							gap: '12px'
						},
						[
							div(
								{
									fontSize: '56px',
									fontWeight: 700,
									color: '#ffffff',
									lineHeight: 1.2
								},
								title
							),
							params.subtitle
								? div(
										{
											fontSize: '24px',
											color: 'rgba(255, 255, 255, 0.7)'
										},
										params.subtitle
									)
								: null
						].filter(Boolean)
					)
				].filter(Boolean)
			),
			div(
				{
					display: 'flex',
					justifyContent: 'flex-end'
				},
				[
					div(
						{
							fontSize: '20px',
							color: 'rgba(255, 255, 255, 0.5)'
						},
						config.domain?.replace(/^https?:\/\//, '') || ''
					)
				]
			)
		]
	);
};
const articleTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #2d3748 0%, #1a202c 100%)';
	return div(
		{
			...baseContainerStyle,
			background: gradient,
			justifyContent: 'space-between'
		},
		[
			div(
				{
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'space-between'
				},
				[
					div(
						{
							fontSize: '24px',
							fontWeight: 600,
							color: 'rgba(255, 255, 255, 0.8)'
						},
						siteName
					),
					div(
						{
							display: 'flex',
							alignItems: 'center',
							gap: '24px'
						},
						[
							params.readTime
								? div(
										{
											display: 'flex',
											alignItems: 'center',
											gap: '8px',
											fontSize: '18px',
											color: 'rgba(255, 255, 255, 0.7)'
										},
										`${params.readTime} read`
									)
								: null,
							params.difficulty
								? div(
										{
											padding: '6px 14px',
											background: getDifficultyColor(params.difficulty),
											borderRadius: '16px',
											fontSize: '16px',
											fontWeight: 500,
											color: '#ffffff'
										},
										params.difficulty
									)
								: null
						].filter(Boolean)
					)
				]
			),
			div(
				{
					display: 'flex',
					flexDirection: 'column',
					gap: '20px'
				},
				[
					div(
						{
							fontSize: '52px',
							fontWeight: 700,
							color: '#ffffff',
							lineHeight: 1.3,
							maxWidth: '1000px'
						},
						title
					)
				]
			),
			div(
				{
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'space-between'
				},
				[
					div(
						{
							display: 'flex',
							alignItems: 'center',
							gap: '16px'
						},
						[
							params.author
								? div(
										{
											fontSize: '20px',
											color: 'rgba(255, 255, 255, 0.8)'
										},
										`By ${params.author}`
									)
								: null,
							params.date
								? div(
										{
											fontSize: '18px',
											color: 'rgba(255, 255, 255, 0.6)'
										},
										params.date
									)
								: null
						].filter(Boolean)
					),
					div(
						{
							fontSize: '20px',
							color: 'rgba(255, 255, 255, 0.5)'
						},
						config.domain?.replace(/^https?:\/\//, '') || ''
					)
				]
			)
		]
	);
};
const faqTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #5b21b6 0%, #1e1b4b 100%)';
	return div(
		{
			...baseContainerStyle,
			background: gradient,
			justifyContent: 'space-between'
		},
		[
			div(
				{
					display: 'flex',
					alignItems: 'center'
				},
				[
					div(
						{
							fontSize: '24px',
							fontWeight: 600,
							color: 'rgba(255, 255, 255, 0.8)'
						},
						siteName
					)
				]
			),
			div(
				{
					display: 'flex',
					alignItems: 'center',
					gap: '32px'
				},
				[
					div(
						{
							display: 'flex',
							alignItems: 'center',
							justifyContent: 'center',
							width: '100px',
							height: '100px',
							background: 'rgba(255, 255, 255, 0.1)',
							borderRadius: '50%',
							fontSize: '56px'
						},
						'?'
					),
					div(
						{
							display: 'flex',
							flexDirection: 'column',
							gap: '12px'
						},
						[
							div(
								{
									fontSize: '20px',
									fontWeight: 500,
									color: '#a78bfa',
									textTransform: 'uppercase',
									letterSpacing: '2px'
								},
								'FAQ'
							),
							div(
								{
									fontSize: '52px',
									fontWeight: 700,
									color: '#ffffff',
									lineHeight: 1.2
								},
								title
							)
						]
					)
				]
			),
			div(
				{
					display: 'flex',
					justifyContent: 'flex-end'
				},
				[
					div(
						{
							fontSize: '20px',
							color: 'rgba(255, 255, 255, 0.5)'
						},
						config.domain?.replace(/^https?:\/\//, '') || ''
					)
				]
			)
		]
	);
};
const howtoTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #065f46 0%, #022c22 100%)';
	return div(
		{
			...baseContainerStyle,
			background: gradient,
			justifyContent: 'space-between'
		},
		[
			div(
				{
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'space-between'
				},
				[
					div(
						{
							fontSize: '24px',
							fontWeight: 600,
							color: 'rgba(255, 255, 255, 0.8)'
						},
						siteName
					),
					div(
						{
							display: 'flex',
							alignItems: 'center',
							gap: '8px',
							padding: '8px 16px',
							background: 'rgba(16, 185, 129, 0.2)',
							borderRadius: '20px',
							border: '1px solid rgba(16, 185, 129, 0.4)'
						},
						[
							div(
								{
									fontSize: '16px',
									color: '#10b981',
									fontWeight: 500
								},
								'Step-by-Step Guide'
							)
						]
					)
				]
			),
			div(
				{
					display: 'flex',
					alignItems: 'center',
					gap: '32px'
				},
				[
					div(
						{
							display: 'flex',
							flexDirection: 'column',
							gap: '8px'
						},
						[1, 2, 3].map((num) =>
							div(
								{
									display: 'flex',
									alignItems: 'center',
									justifyContent: 'center',
									width: '40px',
									height: '40px',
									background: num === 1 ? '#10b981' : 'rgba(255, 255, 255, 0.1)',
									borderRadius: '50%',
									fontSize: '20px',
									fontWeight: 600,
									color: '#ffffff'
								},
								String(num)
							)
						)
					),
					div(
						{
							display: 'flex',
							flexDirection: 'column',
							gap: '12px'
						},
						[
							div(
								{
									fontSize: '52px',
									fontWeight: 700,
									color: '#ffffff',
									lineHeight: 1.2
								},
								title
							),
							params.subtitle
								? div(
										{
											fontSize: '24px',
											color: 'rgba(255, 255, 255, 0.7)'
										},
										params.subtitle
									)
								: null
						].filter(Boolean)
					)
				]
			),
			div(
				{
					display: 'flex',
					justifyContent: 'flex-end'
				},
				[
					div(
						{
							fontSize: '20px',
							color: 'rgba(255, 255, 255, 0.5)'
						},
						config.domain?.replace(/^https?:\/\//, '') || ''
					)
				]
			)
		]
	);
};
function getDifficultyColor(difficulty: string) {
	switch (difficulty) {
		case 'Beginner':
			return '#10b981';
		case 'Intermediate':
			return '#f59e0b';
		case 'Advanced':
			return '#ef4444';
		default:
			return '#6b7280';
	}
}
function renderStars(rating: number) {
	const fullStars = Math.floor(rating);
	const hasHalf = rating - fullStars >= 0.5;
	const stars = [];
	for (let i = 0; i < 5; i++) {
		stars.push(
			div(
				{
					fontSize: '28px',
					color: i < fullStars || (i === fullStars && hasHalf) ? '#fbbf24' : 'rgba(255,255,255,0.3)'
				},
				'★'
			)
		);
	}
	return stars;
}
const productTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #1e40af 0%, #7c3aed 100%)';
	return div({ ...baseContainerStyle, background: gradient, justifyContent: 'space-between' }, [
		div(
			{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' },
			[
				div({ fontSize: '24px', fontWeight: 600, color: 'rgba(255, 255, 255, 0.8)' }, siteName),
				params.badge
					? div(
							{
								padding: '8px 20px',
								background: '#ef4444',
								borderRadius: '24px',
								fontSize: '18px',
								fontWeight: 600,
								color: '#ffffff'
							},
							params.badge
						)
					: null
			].filter(Boolean)
		),
		div({ display: 'flex', flexDirection: 'column', gap: '20px' }, [
			div(
				{ fontSize: '56px', fontWeight: 700, color: '#ffffff', lineHeight: 1.2, maxWidth: '900px' },
				title
			),
			div(
				{ display: 'flex', alignItems: 'center', gap: '24px' },
				[
					params.price
						? div({ fontSize: '40px', fontWeight: 700, color: '#fbbf24' }, params.price)
						: null,
					params.rating
						? div({ display: 'flex', alignItems: 'center', gap: '4px' }, renderStars(params.rating))
						: null
				].filter(Boolean)
			)
		]),
		div(
			{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' },
			[
				params.category
					? div(
							{
								padding: '8px 16px',
								background: 'rgba(255, 255, 255, 0.15)',
								borderRadius: '8px',
								fontSize: '18px',
								color: 'rgba(255, 255, 255, 0.9)'
							},
							params.category
						)
					: null,
				div(
					{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.5)' },
					config.domain?.replace(/^https?:\/\//, '') || ''
				)
			].filter(Boolean)
		)
	]);
};
const eventTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #dc2626 0%, #9333ea 100%)';
	return div({ ...baseContainerStyle, background: gradient, justifyContent: 'space-between' }, [
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div({ fontSize: '24px', fontWeight: 600, color: 'rgba(255, 255, 255, 0.8)' }, siteName),
			div(
				{
					padding: '8px 20px',
					background: 'rgba(255, 255, 255, 0.2)',
					borderRadius: '24px',
					fontSize: '16px',
					fontWeight: 500,
					color: '#ffffff',
					border: '1px solid rgba(255, 255, 255, 0.3)'
				},
				'\uD83C\uDF89 EVENT'
			)
		]),
		div({ display: 'flex', flexDirection: 'column', gap: '24px' }, [
			div(
				{ fontSize: '56px', fontWeight: 700, color: '#ffffff', lineHeight: 1.2, maxWidth: '900px' },
				title
			),
			div(
				{ display: 'flex', alignItems: 'center', gap: '32px' },
				[
					params.date
						? div(
								{ fontSize: '24px', color: 'rgba(255, 255, 255, 0.9)' },
								`\uD83D\uDCC5 ${params.date}`
							)
						: null,
					params.location
						? div(
								{ fontSize: '24px', color: 'rgba(255, 255, 255, 0.9)' },
								`\uD83D\uDCCD ${params.location}`
							)
						: null
				].filter(Boolean)
			)
		]),
		div(
			{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' },
			[
				params.price
					? div({ fontSize: '28px', fontWeight: 700, color: '#fbbf24' }, params.price)
					: null,
				div(
					{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.5)' },
					config.domain?.replace(/^https?:\/\//, '') || ''
				)
			].filter(Boolean)
		)
	]);
};
const recipeTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #ea580c 0%, #dc2626 100%)';
	return div({ ...baseContainerStyle, background: gradient, justifyContent: 'space-between' }, [
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div({ fontSize: '24px', fontWeight: 600, color: 'rgba(255, 255, 255, 0.8)' }, siteName),
			div(
				{
					padding: '8px 20px',
					background: 'rgba(255, 255, 255, 0.2)',
					borderRadius: '24px',
					fontSize: '16px',
					fontWeight: 500,
					color: '#ffffff'
				},
				'\uD83C\uDF73 RECIPE'
			)
		]),
		div(
			{ display: 'flex', flexDirection: 'column', gap: '20px' },
			[
				div(
					{
						fontSize: '56px',
						fontWeight: 700,
						color: '#ffffff',
						lineHeight: 1.2,
						maxWidth: '900px'
					},
					title
				),
				params.subtitle
					? div({ fontSize: '24px', color: 'rgba(255, 255, 255, 0.8)' }, params.subtitle)
					: null
			].filter(Boolean)
		),
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div(
				{ display: 'flex', alignItems: 'center', gap: '24px' },
				[
					params.duration
						? div(
								{
									padding: '8px 16px',
									background: 'rgba(255, 255, 255, 0.15)',
									borderRadius: '8px',
									fontSize: '20px',
									color: '#ffffff'
								},
								`⏱️ ${params.duration}`
							)
						: null,
					params.rating
						? div({ display: 'flex', alignItems: 'center', gap: '4px' }, renderStars(params.rating))
						: null
				].filter(Boolean)
			),
			div(
				{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.5)' },
				config.domain?.replace(/^https?:\/\//, '') || ''
			)
		])
	]);
};
const videoTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #0f172a 0%, #1e293b 100%)';
	return div({ ...baseContainerStyle, background: gradient, justifyContent: 'space-between' }, [
		div(
			{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' },
			[
				div({ fontSize: '24px', fontWeight: 600, color: 'rgba(255, 255, 255, 0.8)' }, siteName),
				params.category
					? div(
							{
								padding: '8px 16px',
								background: 'rgba(255, 255, 255, 0.1)',
								borderRadius: '8px',
								fontSize: '16px',
								color: 'rgba(255, 255, 255, 0.8)'
							},
							params.category
						)
					: null
			].filter(Boolean)
		),
		div({ display: 'flex', alignItems: 'center', gap: '32px' }, [
			div(
				{
					display: 'flex',
					alignItems: 'center',
					justifyContent: 'center',
					width: '100px',
					height: '100px',
					background: '#ef4444',
					borderRadius: '50%',
					fontSize: '48px',
					color: '#fff'
				},
				'▶'
			),
			div(
				{ display: 'flex', flexDirection: 'column', gap: '12px' },
				[
					div(
						{
							fontSize: '48px',
							fontWeight: 700,
							color: '#ffffff',
							lineHeight: 1.2,
							maxWidth: '800px'
						},
						title
					),
					params.subtitle
						? div({ fontSize: '22px', color: 'rgba(255, 255, 255, 0.7)' }, params.subtitle)
						: null
				].filter(Boolean)
			)
		]),
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div(
				{ display: 'flex', alignItems: 'center', gap: '24px' },
				[
					params.duration
						? div(
								{
									padding: '8px 16px',
									background: 'rgba(239, 68, 68, 0.8)',
									borderRadius: '8px',
									fontSize: '18px',
									fontWeight: 600,
									color: '#ffffff'
								},
								params.duration
							)
						: null,
					params.author
						? div({ fontSize: '18px', color: 'rgba(255, 255, 255, 0.7)' }, params.author)
						: null
				].filter(Boolean)
			),
			div(
				{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.5)' },
				config.domain?.replace(/^https?:\/\//, '') || ''
			)
		])
	]);
};
const jobTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #059669 0%, #0d9488 100%)';
	return div({ ...baseContainerStyle, background: gradient, justifyContent: 'space-between' }, [
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div({ fontSize: '24px', fontWeight: 600, color: 'rgba(255, 255, 255, 0.8)' }, siteName),
			div(
				{
					padding: '8px 20px',
					background: '#fbbf24',
					borderRadius: '24px',
					fontSize: '16px',
					fontWeight: 600,
					color: '#1f2937'
				},
				'\uD83D\uDCBC HIRING'
			)
		]),
		div(
			{ display: 'flex', flexDirection: 'column', gap: '16px' },
			[
				div(
					{
						fontSize: '56px',
						fontWeight: 700,
						color: '#ffffff',
						lineHeight: 1.2,
						maxWidth: '900px'
					},
					title
				),
				params.category
					? div({ fontSize: '24px', color: 'rgba(255, 255, 255, 0.8)' }, params.category)
					: null
			].filter(Boolean)
		),
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div(
				{ display: 'flex', alignItems: 'center', gap: '24px' },
				[
					params.price
						? div(
								{
									padding: '8px 16px',
									background: 'rgba(255, 255, 255, 0.15)',
									borderRadius: '8px',
									fontSize: '22px',
									fontWeight: 600,
									color: '#ffffff'
								},
								`\uD83D\uDCB0 ${params.price}`
							)
						: null,
					params.location
						? div(
								{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.8)' },
								`\uD83D\uDCCD ${params.location}`
							)
						: null
				].filter(Boolean)
			),
			div(
				{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.5)' },
				config.domain?.replace(/^https?:\/\//, '') || ''
			)
		])
	]);
};
const reviewTemplate: OgTemplateRenderer = ({ title, siteName, config, params }) => {
	const gradient =
		params.gradient ||
		config.defaultGradient ||
		'linear-gradient(135deg, #7c3aed 0%, #db2777 100%)';
	return div({ ...baseContainerStyle, background: gradient, justifyContent: 'space-between' }, [
		div({ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }, [
			div({ fontSize: '24px', fontWeight: 600, color: 'rgba(255, 255, 255, 0.8)' }, siteName),
			div(
				{
					padding: '8px 20px',
					background: 'rgba(255, 255, 255, 0.2)',
					borderRadius: '24px',
					fontSize: '16px',
					fontWeight: 500,
					color: '#ffffff'
				},
				'⭐ REVIEW'
			)
		]),
		div(
			{ display: 'flex', flexDirection: 'column', gap: '24px' },
			[
				div(
					{
						fontSize: '52px',
						fontWeight: 700,
						color: '#ffffff',
						lineHeight: 1.2,
						maxWidth: '900px'
					},
					title
				),
				params.rating
					? div({ display: 'flex', alignItems: 'center', gap: '16px' }, [
							div(
								{ display: 'flex', alignItems: 'center', gap: '6px' },
								renderStars(params.rating)
							),
							div({ fontSize: '32px', fontWeight: 700, color: '#fbbf24' }, `${params.rating}/5`)
						])
					: null
			].filter(Boolean)
		),
		div(
			{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' },
			[
				params.author
					? div(
							{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.8)' },
							`Review by ${params.author}`
						)
					: null,
				div(
					{ fontSize: '20px', color: 'rgba(255, 255, 255, 0.5)' },
					config.domain?.replace(/^https?:\/\//, '') || ''
				)
			].filter(Boolean)
		)
	]);
};
const templates: Record<string, OgTemplateRenderer> = {
	website: websiteTemplate,
	tool: toolTemplate,
	article: articleTemplate,
	faq: faqTemplate,
	howto: howtoTemplate,
	product: productTemplate,
	event: eventTemplate,
	recipe: recipeTemplate,
	video: videoTemplate,
	job: jobTemplate,
	review: reviewTemplate
};
export function getTemplate(type: string): OgTemplateRenderer {
	return Object.hasOwn(templates, type) ? templates[type] : templates.website;
}
