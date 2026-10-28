import type { NamespaceRules, NamespaceState, Params, RpcError } from '../protocol';

/** A function a Wallet performs for one namespace. */
export interface WalletFunction {
  /** Read-only functions are performed again when repeated (core §6). */
  readOnly?: boolean;
  /**
   * The error to answer instead of performing the function, or null: invalid
   * params (-32602), an account that is not approved (4100), another chain
   * (4901). Wallets and DApp sides run the same check.
   */
  check(params: Params | undefined, state: NamespaceState): RpcError | null;
}

/** One network specification (spec/networks/*.md). */
export interface Network extends NamespaceRules {
  namespace: string;
  functions: Record<string, WalletFunction>;
}
