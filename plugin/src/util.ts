export function errorToMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function extensionOf(path: string): string {
  const fileName = path.split("/").pop() ?? path;
  const dot = fileName.lastIndexOf(".");
  return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : "";
}
