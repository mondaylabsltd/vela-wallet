/**
 * A single-flight re-entry lock with generation tokens.
 *
 * `begin()` acquires the lock (or returns `null` if already held) and hands back
 * a generation token. `end(token)` releases only if that token is still the
 * current generation. `cancel()` force-releases AND invalidates the current
 * holder, so the holder's later `end()` is a no-op.
 *
 * Why the generation token (issue #91): a cancelled send must release the lock
 * immediately so a retry starts instead of silently no-op'ing — but the
 * cancelled send's promise is still running and will eventually hit its own
 * `end()` in a `finally`. Without a generation, that stale `end()` would clear a
 * *newer* send's lock and open a double-submit window. The generation makes the
 * stale `end()` a no-op while keeping the newer send locked.
 */
export interface ReentryLock {
  /** Acquire. Returns a generation token, or `null` if already held. */
  begin(): number | null;
  /** Release iff `token` is the current generation. Returns whether it released. */
  end(token: number): boolean;
  /** Force-release and invalidate the current holder (its later `end()` no-ops). */
  cancel(): void;
  /** Whether the lock is currently held. */
  readonly busy: boolean;
}

export function createReentryLock(): ReentryLock {
  let held = false;
  let generation = 0;
  return {
    begin() {
      if (held) return null;
      held = true;
      return ++generation;
    },
    end(token) {
      if (held && token === generation) {
        held = false;
        return true;
      }
      return false;
    },
    cancel() {
      generation += 1; // invalidate the current holder's pending end()
      held = false;
    },
    get busy() {
      return held;
    },
  };
}
