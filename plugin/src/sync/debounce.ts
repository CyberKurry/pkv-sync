export class Debouncer {
  private timer: number | null = null;

  constructor(
    private delayMs: number,
    private callback: () => void
  ) {}

  trigger(): void {
    this.cancel();
    this.timer = window.setTimeout(() => {
      this.timer = null;
      this.callback();
    }, this.delayMs);
  }

  cancel(): void {
    if (this.timer !== null) {
      window.clearTimeout(this.timer);
      this.timer = null;
    }
  }

  isPending(): boolean {
    return this.timer !== null;
  }
}
