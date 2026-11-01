import { PromiseChain } from "./util/promise-chain";

export interface CommitHooks {
  beforeSave(): undefined;
  publish(next: unknown): undefined;
}

export class DataCommitError extends Error {
  constructor(
    public readonly state: "restored" | "uncertain" | "committed",
    public readonly original: unknown,
    public readonly recovery: unknown = undefined
  ) {
    super(`data_commit_${state}`);
    this.name = "DataCommitError";
  }
}

function jsonCopy(value: unknown): unknown {
  const encoded = JSON.stringify(value);
  return encoded === undefined ? null : (JSON.parse(encoded) as unknown);
}

function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const object = value as Record<string, unknown>;
    return `{${Object.keys(object)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

export class SerializedPluginDataStore {
  private readonly writeChain = new PromiseChain();

  constructor(
    private load: () => Promise<unknown>,
    private save: (data: unknown) => Promise<void>
  ) {}

  private enqueue<T>(work: () => Promise<T>): Promise<T> {
    return this.writeChain.enqueue(work);
  }

  async update(updater: (data: unknown) => unknown): Promise<void> {
    await this.enqueue(async () => {
      const current = await this.load();
      const next = await updater(current);
      if (next === undefined) {
        throw new Error("Plugin data update returned undefined");
      }
      await this.save(next);
    });
  }

  async read<T>(reader: (data: unknown) => T | Promise<T>): Promise<T> {
    // Reading is also a chain task (not a bare tail-await): enqueueing
    // guarantees the read observes every update already submitted, and
    // any submitted after it queue behind.
    return this.enqueue(async () => reader(await this.load()));
  }

  async commitWithRecovery(
    updater: (raw: unknown) => unknown | Promise<unknown>,
    hooks: CommitHooks
  ): Promise<void> {
    return this.enqueue(async () => {
      let original: unknown;
      try {
        original = jsonCopy(await this.load());
      } catch (error) {
        throw new DataCommitError("uncertain", error);
      }
      const proposed = await updater(jsonCopy(original));
      if (proposed === undefined) {
        throw new Error("Plugin data update returned undefined");
      }
      const next = jsonCopy(proposed);
      hooks.beforeSave();
      try {
        await this.save(next);
      } catch (error) {
        const isOriginal = async (): Promise<boolean> => {
          const actual = jsonCopy(await this.load());
          return stableJson(actual) === stableJson(original);
        };
        let restored = false;
        let recovery: unknown;
        try {
          restored = await isOriginal();
        } catch (readError) {
          recovery = readError;
        }
        if (!restored) {
          try {
            await this.save(jsonCopy(original));
          } catch (saveError) {
            recovery = saveError;
          }
          try {
            restored = await isOriginal();
          } catch (readError) {
            recovery = readError;
          }
        }
        throw new DataCommitError(restored ? "restored" : "uncertain", error, recovery);
      }
      try {
        hooks.publish(next);
      } catch (error) {
        throw new DataCommitError("committed", error);
      }
    });
  }
}
