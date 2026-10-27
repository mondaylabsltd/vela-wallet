/**
 * Amounts between base units and what a person types and reads. Pure, so
 * every screen and record agrees, and testable on its own.
 */

/** `1234500000000000000` with 18 decimals → `1.2345`. */
export function formatUnits(amount: string | bigint, decimals: number): string {
	const value = typeof amount === 'bigint' ? amount : BigInt(amount);
	const base = 10n ** BigInt(decimals);
	const whole = value / base;
	const fraction = (value % base).toString().padStart(decimals, '0').replace(/0+$/, '');
	const shown = fraction.length > 6 ? fraction.slice(0, 6) : fraction;
	return shown ? `${whole.toLocaleString('en-US')}.${shown}` : whole.toLocaleString('en-US');
}

/** `1.5` with 18 decimals → base units, or `null` when it is not an amount. */
export function parseUnits(text: string, decimals: number): bigint | null {
	const clean = text.trim().replace(/,/g, '');
	if (!/^\d*\.?\d*$/.test(clean) || clean === '' || clean === '.') return null;
	const [whole, fraction = ''] = clean.split('.');
	if (fraction.length > decimals) return null;
	return (
		BigInt(whole || '0') * 10n ** BigInt(decimals) +
		BigInt((fraction + '0'.repeat(decimals)).slice(0, decimals) || '0')
	);
}

/** Base units as a plain decimal string (the records' `value`). */
export function plainUnits(amount: string, decimals: number): string {
	const value = BigInt(amount);
	const base = 10n ** BigInt(decimals);
	const fraction = (value % base).toString().padStart(decimals, '0').replace(/0+$/, '');
	return fraction ? `${value / base}.${fraction}` : `${value / base}`;
}
