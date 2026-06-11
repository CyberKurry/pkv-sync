export class SerializedPluginDataStore {
  private writeChain: Promise<void> = Promise.resolve();

  constructor(
    private load: () => Promise<unknown>,
    private save: (data: unknown) => Promise<void>
  ) {}

  async update(
    updater: (data: unknown) => unknown
  ): Promise<void> {
    const run = this.writeChain.then(async () => {
      const current = await this.load();
      const next = await updater(current);
      if (next === undefined) {
        throw new Error("Plugin data update returned undefined");
      }
      await this.save(next);
    });
    this.writeChain = run.catch(() => undefined);
    await run;
  }

  async read<T>(reader: (data: unknown) => T | Promise<T>): Promise<T> {
    await this.writeChain;
    return reader(await this.load());
  }
}
