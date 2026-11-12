/**
 * 0b readiness gate (spec §3.4): Obsidian loads community plugins
 * BEFORE the vault file index (`await plugins.initialize()` precedes
 * `vault.load()`), so a sync started from onload() can observe an empty
 * vault. workspace.layoutReady flips only after vault.load() completed
 * the first enumeration, making it the explicit readiness signal.
 */
export function engineStartAllowed(
  workspace: Pick<{ layoutReady: boolean }, "layoutReady">
): boolean {
  return workspace.layoutReady;
}
