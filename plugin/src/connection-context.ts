export interface ConnectionIdentity {
  serverUrl: string;
  deploymentKey: string;
  token: string;
  userId: string;
  username: string;
}

export interface ConnectionStamp {
  epoch: number;
  connection: Readonly<ConnectionIdentity>;
}

export interface RefreshTicket extends ConnectionStamp {
  request: number;
}

export interface MigrationTicket extends ConnectionStamp {
  id: symbol;
  selectionRevision: number;
  vaultId: string;
  phase: "transferring" | "committing" | "committed";
}

export class OperationStoppedError extends Error {
  constructor(
    public readonly reason: "cancelled" | "scope_changed" | "superseded"
  ) {
    super(reason);
    this.name = "OperationStoppedError";
  }
}

export interface ConnectionContextContract {
  runLocal<T>(work: () => T | Promise<T>): Promise<T>;
  snapshotLocked(): ConnectionStamp;
  isConnectionCurrent(stamp: ConnectionStamp): boolean;
  replaceConnectionLocked(next: ConnectionIdentity): boolean;
  replaceSelectionLocked(vaultId: string): boolean;
  captureRefresh(): Promise<RefreshTicket>;
  isRefreshCurrent(ticket: RefreshTicket): boolean;
  beginMigrationLocked(): MigrationTicket;
  assertMigration(ticket: MigrationTicket): void;
  beginCommitLocked(ticket: MigrationTicket): void;
  adoptMigrationSelectionLocked(ticket: MigrationTicket, vaultId: string): void;
  finishMigrationLocked(ticket: MigrationTicket): boolean;
  isCompletionCurrent(ticket: MigrationTicket): boolean;
  readonly migrationPaused: boolean;
}

function copyIdentity(value: ConnectionIdentity): Readonly<ConnectionIdentity> {
  return Object.freeze({
    serverUrl: value.serverUrl,
    deploymentKey: value.deploymentKey,
    token: value.token,
    userId: value.userId,
    username: value.username
  });
}

function sameIdentity(a: ConnectionIdentity, b: ConnectionIdentity): boolean {
  return (
    a.serverUrl === b.serverUrl &&
    a.deploymentKey === b.deploymentKey &&
    a.token === b.token &&
    a.userId === b.userId &&
    a.username === b.username
  );
}

/**
 * In-memory authority for connection identity, vault selection and migration
 * ownership. Nothing here is persisted: the stamps exist so a network result
 * can be checked against the connection it was requested for before it is
 * published into settings or the engine.
 *
 * Every `*Locked` method must run inside `runLocal`. The FIFO tail is what
 * makes a check-then-assign pair atomic with respect to another connection or
 * selection change.
 */
export class ConnectionContext implements ConnectionContextContract {
  private tail: Promise<void> = Promise.resolve();
  private epoch = 0;
  private request = 0;
  private selectionRevision = 0;
  private owner: symbol | null = null;
  private latestMigrationId: symbol | null = null;
  private identity: Readonly<ConnectionIdentity>;

  constructor(
    connection: ConnectionIdentity,
    private selectedVaultId: string
  ) {
    this.identity = copyIdentity(connection);
  }

  runLocal<T>(work: () => T | Promise<T>): Promise<T> {
    const run = this.tail.then(work);
    this.tail = run.then(
      () => undefined,
      () => undefined
    );
    return run;
  }

  snapshotLocked(): ConnectionStamp {
    return { epoch: this.epoch, connection: copyIdentity(this.identity) };
  }

  isConnectionCurrent(stamp: ConnectionStamp): boolean {
    return this.matches(stamp);
  }

  replaceConnectionLocked(next: ConnectionIdentity): boolean {
    if (sameIdentity(this.identity, next)) return false;
    this.identity = copyIdentity(next);
    this.epoch += 1;
    this.request += 1;
    this.owner = null;
    return true;
  }

  replaceSelectionLocked(vaultId: string): boolean {
    if (vaultId === this.selectedVaultId) return false;
    this.selectedVaultId = vaultId;
    this.selectionRevision += 1;
    this.owner = null;
    return true;
  }

  captureRefresh(): Promise<RefreshTicket> {
    return this.runLocal(() => ({ ...this.snapshotLocked(), request: ++this.request }));
  }

  isRefreshCurrent(ticket: RefreshTicket): boolean {
    return this.matches(ticket) && ticket.request === this.request;
  }

  beginMigrationLocked(): MigrationTicket {
    const id = Symbol("migration");
    this.owner = id;
    this.latestMigrationId = id;
    return {
      ...this.snapshotLocked(),
      id,
      selectionRevision: this.selectionRevision,
      vaultId: this.selectedVaultId,
      phase: "transferring"
    };
  }

  assertMigration(ticket: MigrationTicket): void {
    if (
      !this.matches(ticket) ||
      ticket.selectionRevision !== this.selectionRevision ||
      ticket.vaultId !== this.selectedVaultId
    ) {
      throw new OperationStoppedError("scope_changed");
    }
    if (ticket.id !== this.owner) throw new OperationStoppedError("superseded");
  }

  beginCommitLocked(ticket: MigrationTicket): void {
    this.assertMigration(ticket);
    if (ticket.phase !== "transferring") throw new Error("invalid_migration_phase");
    ticket.phase = "committing";
  }

  adoptMigrationSelectionLocked(ticket: MigrationTicket, vaultId: string): void {
    this.assertMigration(ticket);
    if (ticket.phase !== "committing") throw new Error("invalid_migration_phase");
    this.selectedVaultId = vaultId;
    this.selectionRevision += 1;
    ticket.vaultId = vaultId;
    ticket.selectionRevision = this.selectionRevision;
    ticket.phase = "committed";
  }

  finishMigrationLocked(ticket: MigrationTicket): boolean {
    if (this.owner !== ticket.id || !this.matches(ticket)) return false;
    this.owner = null;
    return true;
  }

  isCompletionCurrent(ticket: MigrationTicket): boolean {
    return (
      ticket.phase === "committed" &&
      this.matches(ticket) &&
      ticket.selectionRevision === this.selectionRevision &&
      ticket.vaultId === this.selectedVaultId &&
      ticket.id === this.latestMigrationId
    );
  }

  get migrationPaused(): boolean {
    return this.owner !== null;
  }

  private matches(ticket: ConnectionStamp): boolean {
    return ticket.epoch === this.epoch && sameIdentity(ticket.connection, this.identity);
  }
}
