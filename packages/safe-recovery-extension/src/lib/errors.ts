import type { SerializedProviderError } from './types';

export class ProviderRpcError extends Error {
  readonly code: number;
  readonly data?: unknown;

  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.name = 'ProviderRpcError';
    this.code = code;
    this.data = data;
  }
}

export function providerError(code: number, message: string, data?: unknown): ProviderRpcError {
  return new ProviderRpcError(code, message, data);
}

export function serializeError(error: unknown): SerializedProviderError {
  const candidate = error as { code?: unknown; message?: unknown; data?: unknown };
  const code = typeof candidate?.code === 'number' ? candidate.code : -32603;
  const message = typeof candidate?.message === 'string' ? candidate.message : 'Internal error';
  return { code, message, ...(candidate?.data === undefined ? {} : { data: candidate.data }) };
}
