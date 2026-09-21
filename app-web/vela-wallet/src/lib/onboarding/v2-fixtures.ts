/**
 * Every v2 screen state, as the core would emit it.
 *
 * These are `CreateView` values, not a parallel presentation model — which is
 * the point. Spec 014's gallery had its own state vocabulary because there was
 * no core to render from; now there is, so a fixture is just a view the machine
 * could plausibly produce, and a screen that renders a fixture wrong renders
 * the real thing wrong too.
 *
 * Dev-only. Nothing here ships.
 */

import type { CreateKeyRow } from './generated/CreateKeyRow';
import type { CreateView } from './generated/CreateView';
import type { PromptKind } from './generated/PromptKind';

/**
 * What the core would report this shape to be — a mirror of
 * `vela_core::passkey::reported_method`, so a fixture is a row the machine
 * could really emit rather than a combination it never produces. A fixture that
 * disagrees with the core is a gallery that lies about the screen (issue 207).
 */
function reportedKind(attachment: string, transports: string): CreateKeyRow['kind'] | undefined {
	const has = (hint: string) => transports.split(',').some((t) => t.trim() === hint);
	if (has('usb') || has('nfc') || has('ble')) return 'security_key';
	if (has('hybrid') && (attachment === 'cross-platform' || !has('internal'))) return 'hybrid';
	if (has('internal') || attachment === 'platform') return 'platform';
	if (attachment === 'cross-platform') return 'security_key';
	return undefined;
}

function key(over: Partial<CreateKeyRow> = {}): CreateKeyRow {
	const row: CreateKeyRow = {
		name: 'Everyday wallet',
		authenticator_attachment: 'platform',
		transports: 'internal,hybrid',
		confirmed: true,
		synced: true,
		synced_known: true,
		// A real, resolvable AAGUID by default: the gallery should show the
		// case people actually see (a named vault with its own mark), and the
		// unknown-provider fallback is one override away.
		aaguid: 'fbfc3007-154e-4ecc-8c0b-6e020557d7bd',
		provider_name: 'Apple Passwords',
		method: 'platform',
		kind: 'platform',
		...over
	};
	// The report decides, unless the fixture pinned a kind itself.
	return 'kind' in over
		? row
		: {
				...row,
				kind: reportedKind(row.authenticator_attachment, row.transports) ?? row.method
			};
}

function view(over: Partial<CreateView> = {}): CreateView {
	return {
		stage: 'form',
		name: '',
		name_editable: true,
		name_too_long: false,
		acks: [false, false],
		can_submit: false,
		submit_label: 'create',
		busy: false,
		status: null,
		show_start_over: false,
		address: null,
		sync_error_detail: null,
		can_go_back: true,
		keys: [],
		can_add_key: false,
		can_finish: false,
		needs_second_key: false,
		...over
	};
}

export type CreateFixture = { code: string; label: string; view: CreateView };

export const CREATE_FIXTURES: CreateFixture[] = [
	{ code: 'F1', label: 'Name · empty', view: view() },
	{
		code: 'F2',
		label: 'Name · ready',
		view: view({ name: 'Everyday wallet', acks: [true, true], can_submit: true })
	},
	{
		code: 'F3',
		label: 'Name · too long',
		view: view({ name: '十个汉字就超过了预算啦', acks: [true, true], name_too_long: true })
	},
	{
		code: 'F4',
		label: 'Name · resume after a dead passkey',
		view: view({
			name: 'Everyday wallet',
			acks: [true, true],
			can_submit: true,
			submit_label: 'finish_verify',
			show_start_over: true,
			status: 'verify_cancelled'
		})
	},
	{
		code: 'K0',
		label: 'Keys · empty, methods expanded',
		// Where the create flow now lands from the name screen: no key yet, the
		// three add methods held open so the FIRST key's method is the person's
		// choice (the Xiaomi lock-out fix, 2026-08-26).
		view: view({
			stage: 'add_keys',
			name: 'Everyday wallet',
			keys: [],
			can_add_key: true
		})
	},
	{
		code: 'K1',
		label: 'Keys · one synced key',
		view: view({
			stage: 'add_keys',
			name: 'Everyday wallet',
			keys: [key()],
			can_add_key: true,
			can_finish: true
		})
	},
	{
		code: 'K2',
		label: 'Keys · blocked, sole device-bound key',
		view: view({
			stage: 'add_keys',
			name: 'Everyday wallet',
			keys: [key({ synced: false, transports: 'internal' })],
			can_add_key: true,
			needs_second_key: true
		})
	},
	{
		code: 'K3',
		label: 'Keys · three, mixed methods',
		view: view({
			stage: 'add_keys',
			name: 'Everyday wallet',
			keys: [
				key(),
				key({
					name: 'Key 2',
					method: 'hybrid',
					authenticator_attachment: 'cross-platform',
					transports: 'hybrid,internal',
					aaguid: '',
					provider_name: ''
				}),
				// The degradation path, on purpose: a hardware key is not in the
				// provider catalog, so this row must fall back to its shape glyph
				// and the generic line.
				key({
					name: 'Key 3',
					method: 'security_key',
					synced: false,
					authenticator_attachment: 'cross-platform',
					transports: 'usb',
					aaguid: '',
					provider_name: ''
				})
			],
			can_add_key: true,
			can_finish: true
		})
	},
	{
		code: 'K4',
		label: 'Keys · one unconfirmed row',
		view: view({
			stage: 'add_keys',
			name: 'Everyday wallet',
			keys: [key(), key({ name: 'Key 2', confirmed: false })],
			can_add_key: true
		})
	},
	{
		code: 'K5',
		label: 'Keys · at the cap of seven',
		view: view({
			stage: 'add_keys',
			name: 'Everyday wallet',
			keys: Array.from({ length: 7 }, (_, i) =>
				key({ name: i === 0 ? 'Everyday wallet' : `Key ${i + 1}` })
			),
			can_add_key: false,
			can_finish: true
		})
	},
	{
		code: 'P1',
		label: 'Progress · verifying',
		view: view({ stage: 'add_keys', busy: true, status: 'verifying_identity', keys: [key()] })
	},
	{
		code: 'P2',
		label: 'Progress · deriving the address',
		view: view({
			stage: 'add_keys',
			busy: true,
			status: 'computing_address',
			keys: [key(), key({ name: 'Key 2' })]
		})
	},
	{
		code: 'P3',
		label: 'Progress · writing the index',
		view: view({ stage: 'add_keys', busy: true, status: 'syncing_key', keys: [key()] })
	},
	{
		code: 'R1',
		label: 'Retry · the publish never landed',
		view: view({
			stage: 'sync_failed',
			name: 'Everyday wallet',
			keys: [key()],
			sync_error_detail: 'Register failed: 503'
		})
	},
	{
		code: 'D1',
		label: 'Done · one key',
		view: view({
			stage: 'created',
			name: 'Everyday wallet',
			keys: [key()],
			address: '0x71C7A4E9b2F03D8cA51e7F6d92B4c8035E9A3F1c',
			can_go_back: false
		})
	},
	{
		code: 'D2',
		label: 'Done · three keys',
		view: view({
			stage: 'created',
			name: 'Everyday wallet',
			keys: [
				key(),
				// Shaped as the authenticators really report themselves, so the
				// three rows differ where the screen differs: a phone reached by
				// a code, then a hardware key the catalog cannot name.
				key({
					name: 'Key 2',
					method: 'hybrid',
					authenticator_attachment: 'cross-platform',
					transports: 'hybrid,internal',
					aaguid: '',
					provider_name: ''
				}),
				key({
					name: 'Key 3',
					method: 'security_key',
					synced: false,
					authenticator_attachment: 'cross-platform',
					transports: 'usb',
					aaguid: '',
					provider_name: ''
				})
			],
			address: '0x88cCA0B0F1C0e2F3a4B5C6d7E8f90A1b2C3d6894',
			can_go_back: false
		})
	}
];

/**
 * Every prompt the two machines can raise.
 *
 * Nine, not spec 014's eighteen. The other nine — network, timeout, server,
 * account-not-found and the rest — were drawn as separate outcome cards, but
 * the core never says which of them happened: a transport failure and a 503
 * both arrive as `CreateFailed { detail }` or `SignInFailed { detail }` with
 * the platform's own words. Rendering them as distinct screens would mean the
 * shell classifying error strings, which is the one thing the architecture is
 * built to prevent. The taxonomy survives as COPY (`onboarding.common.*`) for
 * a shell that is handed a classification; it is not a state the core emits.
 */
export const PROMPT_FIXTURES: { code: string; label: string; kind: PromptKind }[] = [
	{ code: 'E1', label: 'No passkey support (create)', kind: { type: 'not_supported_create' } },
	{ code: 'E2', label: 'No passkey support (sign-in)', kind: { type: 'not_supported_login' } },
	{ code: 'E3', label: 'No usable passkey on this device', kind: { type: 'not_discoverable' } },
	{ code: 'E4', label: 'Incompatible provider (create)', kind: { type: 'incompatible_create' } },
	{ code: 'E5', label: 'Incompatible provider (sign-in)', kind: { type: 'incompatible_login' } },
	{
		code: 'E6',
		label: 'Creation failed — the platform’s own words',
		kind: { type: 'create_failed', detail: 'Register failed: 503 Service Unavailable' }
	},
	{
		code: 'E7',
		label: 'Recovery offer (the one confirmable prompt)',
		kind: { type: 'recover_offer' }
	},
	{ code: 'E8', label: 'Recovery did not pin one key', kind: { type: 'recover_failed' } },
	{
		code: 'E9',
		label: 'Sign-in failed',
		kind: {
			type: 'sign_in_failed',
			detail: 'NotAllowedError: The operation either timed out or was not allowed.'
		}
	}
];
