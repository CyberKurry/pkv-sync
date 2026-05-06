export class SerializedPluginDataStore {
  private writeChain: Promise<void> = Promise.resolve();

  constructor(
    private load: () => Promise<unknown>,
    private save: (data: unknown) => Promise<void>
  ) {}

  async update(
    updater: (data: unknown) => unknown | Promise<unknown>
  ): Promise<void> {
    const run = this.writeChain.then(async () => {
      const current = await this.load();
      const next = await updater(current);
      await this.save(next);
    });
    this.writeChain = run.catch(() => undefined);
    await run;
  }
}
