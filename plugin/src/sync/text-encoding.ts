export const TEXT_ENCODER = new TextEncoder();

export function textByteLength(value: string): number {
  return TEXT_ENCODER.encode(value).byteLength;
}
