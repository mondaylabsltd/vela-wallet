/**
 * What a chain must have before Vela can run on it.
 *
 * This is the WALLET's own admission bar, not a list invented for this page:
 * the twelve addresses and their order are `REQUIRED_CONTRACTS` in
 * `rust/crates/vela-core/src/app/network_admin.rs`, and the P-256 probe is the
 * wallet's `P256_PRECOMPILE` / `VALID_P256_CALL`. `required-contracts.test.ts`
 * reads that Rust file off disk and fails the moment the two drift, so this
 * page can never call a chain ready that the wallet would refuse.
 *
 * A chain missing ANY of these can accept deposits the wallet can never sign
 * out of — which is why the check runs before anything is deployed, and why
 * the verdict comes before the instructions.
 */

/** How a missing contract gets onto a chain. */
export type DeployMethod =
	/** A keyless, pre-signed transaction: fund its one-time sender, broadcast. Anyone can do it. */
	| 'presigned'
	/** Only its owner can deploy it (the Safe singleton factory: Safe signs per chain). */
	| 'external'
	/** CREATE2 through one of the two factories. Any funded account can do it. */
	| 'create2'
	/** No step of its own: another contract's constructor deploys it. */
	| 'with-factory';

export type Factory = 'arachnid' | 'safeSingletonFactory';

export interface RequiredContract {
	/** Stable key — also the key into `deployment-data.json` for CREATE2 contracts. */
	key: string;
	/** The wallet's name for it, verbatim. */
	name: string;
	address: `0x${string}`;
	method: DeployMethod;
	/** For `create2`: which factory's calldata this is. */
	factory?: Factory;
	/** For `with-factory`: the key whose deployment brings this one along. */
	arrivesWith?: string;
	/**
	 * Only a wallet holding more than one passkey needs this contract. A chain
	 * without it still runs a one-key wallet completely; it just cannot deploy
	 * a wallet whose address was derived from two to seven keys.
	 */
	multiKeyOnly?: true;
	/** One line a person can act on. */
	what: string;
}

export const ARACHNID_PROXY: `0x${string}` = '0x4e59b44847b379578588920cA78FbF26c0B4956C';
export const SAFE_SINGLETON_FACTORY: `0x${string}` = '0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7';

/** In the wallet's order, which is also dependency order: factories first. */
export const REQUIRED_CONTRACTS: readonly RequiredContract[] = [
	{
		key: 'arachnidProxy',
		name: 'Deterministic Deployment Proxy',
		address: ARACHNID_PROXY,
		method: 'presigned',
		what: 'The CREATE2 factory the EntryPoint and two Safe modules deploy through.'
	},
	{
		key: 'safeSingletonFactory',
		name: 'Safe Singleton Factory',
		address: SAFE_SINGLETON_FACTORY,
		method: 'external',
		what: "Safe's own CREATE2 factory. Safe deploys it per chain on request — nobody else can."
	},
	{
		key: 'multicall3',
		name: 'Multicall3',
		address: '0xcA11bde05977b3631167028862bE2a173976CA11',
		method: 'presigned',
		what: 'One round trip for every balance the wallet reads.'
	},
	{
		key: 'entryPoint',
		name: 'EntryPoint v0.7',
		address: '0x0000000071727De22E5E9d8BAf0edAc6f37da032',
		method: 'create2',
		factory: 'arachnid',
		what: 'The ERC-4337 contract every Vela operation is submitted to.'
	},
	{
		key: 'safeSingleton',
		name: 'Safe L2',
		address: '0x29fcB43b46531BcA003ddC8FCB67FFE91900C762',
		method: 'create2',
		factory: 'safeSingletonFactory',
		what: 'The account contract itself.'
	},
	{
		key: 'safeProxyFactory',
		name: 'Safe Proxy Factory',
		address: '0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67',
		method: 'create2',
		factory: 'safeSingletonFactory',
		what: 'What turns a passkey into the same address on every chain.'
	},
	{
		key: 'safe4337Module',
		name: 'Safe 4337 Module',
		address: '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226',
		method: 'create2',
		factory: 'arachnid',
		what: 'Lets the account speak ERC-4337.'
	},
	{
		key: 'safeModuleSetup',
		name: 'Safe Module Setup',
		address: '0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47',
		method: 'create2',
		factory: 'arachnid',
		what: 'Enables that module the moment an account is created.'
	},
	{
		key: 'webAuthnSigner',
		name: 'WebAuthn Signer',
		address: '0x94a4F6affBd8975951142c3999aEAB7ecee555c2',
		method: 'create2',
		factory: 'safeSingletonFactory',
		what: 'Verifies passkey signatures for the account.'
	},
	{
		key: 'multiSend',
		name: 'MultiSend',
		address: '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526',
		method: 'create2',
		factory: 'safeSingletonFactory',
		what: 'Batches several calls into one operation.'
	},
	{
		key: 'safePasskeySignerFactory',
		name: 'Safe Passkey Signer Factory',
		address: '0x1d31F259eE307358a26dFb23EB365939E8641195',
		method: 'create2',
		factory: 'safeSingletonFactory',
		multiKeyOnly: true,
		what: 'Only for a wallet with more than one passkey: it makes the signer contract for each key beyond the first. Deploying it also brings the singleton below.'
	},
	{
		key: 'safePasskeySignerSingleton',
		name: 'Safe Passkey Signer Singleton',
		address: '0x4E27b51350e6c2083EE19011120F50DAfEc5CA50',
		method: 'with-factory',
		arrivesWith: 'safePasskeySignerFactory',
		multiKeyOnly: true,
		what: 'The code every per-key signer runs. The factory’s constructor deploys it, so it needs no step of its own.'
	}
] as const;

export function requiredContract(key: string): RequiredContract {
	const found = REQUIRED_CONTRACTS.find((c) => c.key === key);
	if (!found) throw new Error(`unknown required contract: ${key}`);
	return found;
}

/**
 * EIP-7951 / RIP-7212 — the P-256 precompile at `0x100`.
 *
 * This one is different in kind from the twelve above. Its address is baked
 * into every Vela address: the account's setup calldata names `0x100` as the
 * signature verifier, and the address is derived from that calldata. A chain
 * without the precompile cannot be fixed by deploying anything — swapping in a
 * verifier contract would change every address, on every chain, for everyone.
 * So the verdict here is binary, and it is the chain's to change, not ours.
 */
export const P256_PRECOMPILE: `0x${string}` = '0x0000000000000000000000000000000000000100';

/** sha256("test") signed with a known P-256 key. A working precompile returns 1. */
export const VALID_P256_CALL =
	'0x' +
	'9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08' +
	'7bf0e18d07660f15994adce5c3836d7bd6167cdb5726f631098f433ebe0be9c0' +
	'3936edbe5c791477e714e58244afb690b9b88b833ff4acdf0fbd1b28bf0b1182' +
	'3be8cbcb3f590087711ae5ed74b9cd06a88058d0bbe700b5f0ec5a1bfac15592' +
	'f989ef9bfaae0fee03c36625e88eae99806a879d813411f876e7e03a2ffd8314';

/**
 * EIP-7951 is the P-256 precompile on Ethereum (Fusaka, December 2025) and
 * supersedes RIP-7212 with the same interface at the same address; rollups ship
 * it under the RIP-7212 name. The EIP page covers both.
 */
export const P256_PRECOMPILE_URL = 'https://eips.ethereum.org/EIPS/eip-7951';
