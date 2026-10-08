/**
 * The letters a token circle draws under its logo: the ticker's first three
 * characters, upper-cased ("ETH", "USD" for USDC, "PAT" for pathUSD).
 *
 * The core says the same (`MarkView.glyph`); this copy stays because the
 * fixtures, which ask the core nothing, draw through the same circle. It
 * counts characters, not UTF-16 units, as the core does, and the core's mark
 * vectors check the two agree (`flows/marks.test.ts`).
 */
export function tokenGlyph(ticker: string): string {
	return Array.from(ticker).slice(0, 3).join('').toUpperCase();
}
