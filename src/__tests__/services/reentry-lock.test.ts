/**
 * createReentryLock — the single-flight lock behind the Send confirm slide.
 * Issue #91: a cancelled send must release immediately (retry isn't a silent
 * no-op), while the cancelled promise's stale end() must not clear a newer send.
 */
import { createReentryLock } from '@/services/reentry-lock';

describe('createReentryLock', () => {
  it('begins once, blocks a concurrent begin, and releases with the right token', () => {
    const lock = createReentryLock();
    const a = lock.begin();
    expect(a).not.toBeNull();
    expect(lock.busy).toBe(true);
    expect(lock.begin()).toBeNull(); // already held → no second entry

    expect(lock.end(a!)).toBe(true);
    expect(lock.busy).toBe(false);
    expect(lock.begin()).not.toBeNull(); // free again
  });

  it('end() with a stale token does not release (no clobber of a newer holder)', () => {
    const lock = createReentryLock();
    const a = lock.begin();
    lock.cancel();              // send A cancelled → lock free, A invalidated
    expect(lock.busy).toBe(false);

    const b = lock.begin();     // send B (the retry) acquires
    expect(b).not.toBeNull();
    expect(lock.busy).toBe(true);

    // A's promise finally runs late with its stale token — must be a no-op.
    expect(lock.end(a!)).toBe(false);
    expect(lock.busy).toBe(true); // B is still locked

    expect(lock.end(b!)).toBe(true); // B releases normally
    expect(lock.busy).toBe(false);
  });

  it('cancel() frees the lock so a retry can begin immediately', () => {
    const lock = createReentryLock();
    lock.begin();
    expect(lock.busy).toBe(true);
    lock.cancel();
    expect(lock.busy).toBe(false);
    expect(lock.begin()).not.toBeNull();
  });

  it('the classic #91 sequence: send → cancel → retry → stale finally → retry finally', () => {
    const lock = createReentryLock();
    const gen1 = lock.begin();       // slide
    lock.cancel();                   // tap X to cancel during the ~20s grant wait
    const gen2 = lock.begin();       // slide again → MUST succeed (was a silent no-op before)
    expect(gen2).not.toBeNull();
    expect(lock.end(gen1!)).toBe(false); // cancelled promise settles late → no-op
    expect(lock.busy).toBe(true);        // retry still guarded
    expect(lock.end(gen2!)).toBe(true);  // retry completes
    expect(lock.busy).toBe(false);
  });
});
