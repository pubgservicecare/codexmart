import { invoke } from '@tauri-apps/api/core';

/** Error returned by the Rust services: `{ code, message }`. */
export class ApiError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.code = code;
    this.name = 'ApiError';
  }

  static from(e: unknown): ApiError {
    if (e instanceof ApiError) return e;
    if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
      const err = e as { code: string; message: string };
      return new ApiError(err.code, err.message);
    }
    if (typeof e === 'string') return new ApiError('INTERNAL', e);
    if (e instanceof Error) return new ApiError('INTERNAL', e.message);
    return new ApiError('INTERNAL', 'Something went wrong. Please try again.');
  }
}

type Listener = (e: ApiError) => void;
const listeners = new Set<Listener>();

/** Notified about every failed call (used to return to the sign-in screen). */
export function onApiError(fn: Listener): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/**
 * The only place the UI talks to the backend. Every command lives in the
 * Rust `codexmart_core` services; swapping this transport (e.g. for an HTTP
 * client in a future web admin) does not touch any screen.
 */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    const err = ApiError.from(e);
    listeners.forEach((l) => l(err));
    throw err;
  }
}

export function errorMessage(e: unknown): string {
  return ApiError.from(e).message;
}
