export function errorToMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function extensionOf(path: string): string {
  const fileName = path.split("/").pop() ?? path;
  const dot = fileName.lastIndexOf(".");
  return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : "";
}

export function debugLog(...args: unknown[]): void {
  const env = (window as { process?: { env?: { NODE_ENV?: string } } })
    .process?.env?.NODE_ENV;
  if (env === "development") {
    console.debug(...args);
  }
}
