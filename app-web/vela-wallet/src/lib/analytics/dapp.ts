/**
 * A site's request, as usage statistics name it: the KIND of thing it asked
 * for, from its JSON-RPC method — never its parameters, never the site.
 */
import type { AnalyticsDappKind } from './catalog';

export function dappRequestKind(method: string): AnalyticsDappKind {
	switch (method) {
		case 'eth_requestAccounts':
		case 'wallet_requestPermissions':
			return 'connect';
		case 'personal_sign':
		case 'eth_sign':
			return 'sign_message';
		case 'eth_signTypedData':
		case 'eth_signTypedData_v3':
		case 'eth_signTypedData_v4':
			return 'sign_typed_data';
		case 'eth_sendTransaction':
			return 'send_transaction';
		case 'wallet_sendCalls':
			return 'send_calls';
		case 'wallet_addEthereumChain':
			return 'add_network';
		default:
			return 'other';
	}
}
