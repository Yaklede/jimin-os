type SyncPullOperation = () => Promise<void>;

/**
 * Coalesces overlapping sync pulses while preserving one trailing pull.
 *
 * Server-sent events, focus changes and the reconciliation timer can all fire
 * while a pull is already refreshing projections. Returning only the current
 * promise would drop that newer signal. This coordinator drains one additional
 * pull with the latest operation after the active pull settles.
 */
export class SyncPullCoordinator {
  private requested = false;
  private inFlight: Promise<void> | undefined;
  private latestOperation: SyncPullOperation | undefined;

  request(operation: SyncPullOperation): Promise<void> {
    this.requested = true;
    this.latestOperation = operation;
    if (this.inFlight) return this.inFlight;

    const drain = this.drain();
    this.inFlight = drain;
    return drain;
  }

  private async drain(): Promise<void> {
    try {
      while (this.requested) {
        this.requested = false;
        const operation = this.latestOperation;
        if (!operation) return;
        try {
          await operation();
        } catch (error) {
          if (!this.requested) throw error;
        }
      }
    } finally {
      this.inFlight = undefined;
    }
  }
}
