import { describe, expect, it } from "vitest";
import {
  ConnectionContext,
  OperationStoppedError,
  type ConnectionIdentity
} from "../src/connection-context";

const identityA: ConnectionIdentity = {
  serverUrl: "https://a.example.test",
  deploymentKey: "key-a",
  token: "token-a",
  userId: "user-a",
  username: "a"
};

const identityB: ConnectionIdentity = {
  serverUrl: "https://b.example.test",
  deploymentKey: "key-b",
  token: "token-b",
  userId: "user-b",
  username: "b"
};

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((yes) => {
    resolve = yes;
  });
  return { promise, resolve };
}

describe("ConnectionContext connection scope", () => {
  it("rejects an old response even after A to B to A", async () => {
    const context = new ConnectionContext(identityA, "old-vault");
    const first = await context.captureRefresh();
    await context.runLocal(() => context.replaceConnectionLocked(identityB));
    await context.runLocal(() => context.replaceConnectionLocked(identityA));
    expect(context.isRefreshCurrent(first)).toBe(false);
  });

  it("bumps the epoch when only the token changes on the same server", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const stamp = await context.runLocal(() => context.snapshotLocked());
    const rotated = await context.runLocal(() =>
      context.replaceConnectionLocked({ ...identityA, token: "token-a2" })
    );
    expect(rotated).toBe(true);
    expect(context.isConnectionCurrent(stamp)).toBe(false);
    expect(context.snapshotLocked().epoch).toBe(stamp.epoch + 1);
  });

  it("bumps the epoch when only the deployment key changes", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const stamp = await context.runLocal(() => context.snapshotLocked());
    expect(
      await context.runLocal(() =>
        context.replaceConnectionLocked({ ...identityA, deploymentKey: "key-a2" })
      )
    ).toBe(true);
    expect(context.isConnectionCurrent(stamp)).toBe(false);
  });

  it("reports no change for an identical identity", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const before = context.snapshotLocked();
    expect(
      await context.runLocal(() => context.replaceConnectionLocked({ ...identityA }))
    ).toBe(false);
    expect(context.snapshotLocked().epoch).toBe(before.epoch);
  });

  it("exposes only frozen copies of the identity", () => {
    const context = new ConnectionContext(identityA, "vault");
    const stamp = context.snapshotLocked();
    expect(Object.isFrozen(stamp.connection)).toBe(true);
    expect(stamp.connection.token).toBe("token-a");
  });
});

describe("ConnectionContext refresh tickets", () => {
  it("keeps only the latest refresh current within one scope", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const first = await context.captureRefresh();
    const second = await context.captureRefresh();
    expect(context.isRefreshCurrent(second)).toBe(true);
    expect(context.isRefreshCurrent(first)).toBe(false);
  });

  it("keeps a config refresh current across a vault selection change", async () => {
    const context = new ConnectionContext(identityA, "vault-1");
    const ticket = await context.captureRefresh();
    await context.runLocal(() => context.replaceSelectionLocked("vault-2"));
    expect(context.isRefreshCurrent(ticket)).toBe(true);
  });

  it("serialises queued work behind a blocked gate", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const gate = deferred<void>();
    const events: string[] = [];
    const blocked = context.runLocal(async () => {
      events.push("first:start");
      await gate.promise;
      events.push("first:end");
    });
    const queued = context.runLocal(() => {
      events.push("second");
    });
    await Promise.resolve();
    expect(events).toEqual(["first:start"]);
    gate.resolve();
    await Promise.all([blocked, queued]);
    expect(events).toEqual(["first:start", "first:end", "second"]);
  });

  it("keeps serving queued work after a task throws", async () => {
    const context = new ConnectionContext(identityA, "vault");
    await expect(
      context.runLocal(() => {
        throw new Error("boom");
      })
    ).rejects.toThrow("boom");
    await expect(context.runLocal(() => 7)).resolves.toBe(7);
  });
});

describe("ConnectionContext migration ownership", () => {
  it("does not let an earlier migration finish a newer owner's pause", async () => {
    const context = new ConnectionContext(identityA, "old-vault");
    const first = await context.runLocal(() => context.beginMigrationLocked());
    const second = await context.runLocal(() => context.beginMigrationLocked());
    expect(() => context.assertMigration(first)).toThrow(OperationStoppedError);
    expect(await context.runLocal(() => context.finishMigrationLocked(first))).toBe(
      false
    );
    expect(context.migrationPaused).toBe(true);
    expect(await context.runLocal(() => context.finishMigrationLocked(second))).toBe(
      true
    );
    expect(context.migrationPaused).toBe(false);
  });

  it("stops a migration when the connection is replaced", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const ticket = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.replaceConnectionLocked(identityB));
    expect(() => context.assertMigration(ticket)).toThrow(OperationStoppedError);
    expect(configReason(ticket, context)).toBe("scope_changed");
  });

  it("stops a migration when the selection is replaced", async () => {
    const context = new ConnectionContext(identityA, "vault-1");
    const ticket = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.replaceSelectionLocked("vault-2"));
    expect(() => context.assertMigration(ticket)).toThrow(OperationStoppedError);
    expect(await context.runLocal(() => context.finishMigrationLocked(ticket))).toBe(
      false
    );
  });

  it("marks the ticket superseded when another migration takes over", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const first = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.beginMigrationLocked());
    try {
      context.assertMigration(first);
      throw new Error("expected a stop");
    } catch (error) {
      expect(error).toBeInstanceOf(OperationStoppedError);
      expect((error as OperationStoppedError).reason).toBe("superseded");
    }
  });

  it("adopts the migration's own selection without invalidating itself", async () => {
    const context = new ConnectionContext(identityA, "old-vault");
    const ticket = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.beginCommitLocked(ticket));
    expect(ticket.phase).toBe("committing");
    await context.runLocal(() =>
      context.adoptMigrationSelectionLocked(ticket, "new-vault")
    );
    expect(ticket.phase).toBe("committed");
    expect(ticket.vaultId).toBe("new-vault");
    expect(() => context.assertMigration(ticket)).not.toThrow();

    await context.runLocal(() => context.finishMigrationLocked(ticket));
    expect(context.isCompletionCurrent(ticket)).toBe(true);
  });

  it("rejects a completion from a superseded migration", async () => {
    const context = new ConnectionContext(identityA, "old-vault");
    const first = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.beginCommitLocked(first));
    await context.runLocal(() =>
      context.adoptMigrationSelectionLocked(first, "new-vault")
    );
    const second = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.finishMigrationLocked(first));
    expect(context.isCompletionCurrent(first)).toBe(false);
    expect(() => context.assertMigration(second)).not.toThrow();
  });

  it("requires the transferring phase before committing", async () => {
    const context = new ConnectionContext(identityA, "vault");
    const ticket = await context.runLocal(() => context.beginMigrationLocked());
    await context.runLocal(() => context.beginCommitLocked(ticket));
    expect(() =>
      context.beginCommitLocked(ticket)
    ).toThrow("invalid_migration_phase");
  });
});

function configReason(
  ticket: Parameters<ConnectionContext["assertMigration"]>[0],
  context: ConnectionContext
): string {
  try {
    context.assertMigration(ticket);
    return "none";
  } catch (error) {
    return (error as OperationStoppedError).reason;
  }
}
