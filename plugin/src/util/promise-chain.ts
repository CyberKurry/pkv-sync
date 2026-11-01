/**
 * A minimal FIFO mutex: tasks run in submission order, never interleaved.
 *
 * Extracted from three hand-rolled copies (R7-r3 Dup-4 / T-1):
 * plugin-store's writeChain, connection-context's runLocal tail and the
 * engine's eventChain. The pattern is identical everywhere — chain the
 * work onto the tail, then advance the tail to a settled copy so a
 * rejection never poisons the chain for later tasks.
 *
 * The zero-argument type parameter keeps this usable for `Promise<void>`
 * chains where callers ignore the value.
 */
export class PromiseChain {
  private tail: Promise<unknown> = Promise.resolve();

  /**
   * Enqueue `work` after everything already queued and return its promise.
   * Rejections propagate to the caller only; the chain itself stays
   * usable — the next enqueue runs regardless of whether this one failed.
   */
  enqueue<T>(work: () => T | Promise<T>): Promise<T> {
    const run = this.tail.then(work);
    this.tail = run.then(
      () => undefined,
      () => undefined
    );
    return run;
  }

}
