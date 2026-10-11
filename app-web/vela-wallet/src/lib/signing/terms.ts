/**
 * Every word the core names on a clear-signing result (`ClearTerm`) — an
 * intent, a field label, the "Unlimited" a threshold prints. The core owns the
 * rule (which text is which word); the shell owns the words: each term is the
 * leaf of its key under `componentsUi.signing`, resolved per locale at build
 * time into `SigningMessages.terms`.
 *
 * `satisfies` is the drift guard: a term the core adds, or one it drops, fails
 * the build here rather than showing English on a Chinese sheet.
 */
import type { ClearTerm } from '$lib/core/generated/ClearTerm';

const ALL = {
	intentApprove: true,
	intentApproveNft: true,
	intentApproveAllNfts: true,
	intentAuthorizeSpending: true,
	// The wallet's own copy of its record to Ethereum: its intent ("Copy this
	// wallet's record") and its four rows (network, address, wallet name, keys
	// included) are core terms, so they translate here like any other — no
	// index relabel in the shell. The term keeps its first name on the wire.
	intentBackUpPublicKeys: true,
	intentBorrow: true,
	intentBridge: true,
	intentBurn: true,
	intentBuyNft: true,
	intentClaim: true,
	deployIntent: true,
	intentDeposit: true,
	intentMint: true,
	intentRepay: true,
	intentRevoke: true,
	intentSend: true,
	intentStake: true,
	intentSupply: true,
	intentSwap: true,
	intentTransfer: true,
	intentTransferNft: true,
	intentTransferNfts: true,
	intentUnstake: true,
	intentUnwrap: true,
	intentUnwrapWeth: true,
	intentWithdraw: true,
	intentWrap: true,
	intentWrapEth: true,
	labelAddress: true,
	labelAmount: true,
	labelAmountToSpend: true,
	labelApproved: true,
	labelDeadline: true,
	labelExpires: true,
	labelFrom: true,
	labelMaxSpendingAmount: true,
	labelMinReceived: true,
	labelNetwork: true,
	labelNewContract: true,
	labelNft: true,
	labelNonce: true,
	labelOnBehalfOf: true,
	labelOrder: true,
	labelOperator: true,
	labelOwner: true,
	labelPay: true,
	labelPrice: true,
	labelPublicKeys: true,
	labelQuantities: true,
	labelQuantity: true,
	labelReceived: true,
	labelRecipient: true,
	labelSeller: true,
	labelSpender: true,
	labelTo: true,
	labelToken: true,
	labelTokenId: true,
	labelTokenIds: true,
	labelValidUntil: true,
	// The record's name, which the copy makes public on Ethereum too.
	labelWalletName: true,
	labelYouPay: true,
	labelYouPayMax: true,
	labelYouReceive: true,
	labelYouReceiveMin: true,
	valueUnlimited: true,
	valueAll: true,
	// Spec 093: the headline verbs Activity titles a dApp row with when the
	// request is what it is — a permit, a sign-in, a message, typed data, a
	// blind signature, a batch, a call nobody decoded.
	permitIntent: true,
	signInIntent: true,
	messageIntent: true,
	typedDataIntent: true,
	ethSignIntent: true,
	batchIntent: true,
	intentContractCall: true
} satisfies Record<ClearTerm, true>;

export const CLEAR_TERMS = Object.keys(ALL) as ClearTerm[];

/**
 * The terms that name what a request DOES — the ones a recorded intent can be
 * (083 H2), and the headline verbs of spec 093 (`permitIntent`,
 * `signInIntent`…). Activity titles a dApp row with one, so the wallet page
 * ships these words and not the field labels.
 */
export const INTENT_TERMS = CLEAR_TERMS.filter(
	(term) => term.startsWith('intent') || term.endsWith('Intent')
);
