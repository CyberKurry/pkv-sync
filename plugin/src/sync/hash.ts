import { TEXT_ENCODER } from "./text-encoding";
const HEX_BYTES = Array.from({ length: 256 }, (_, i) =>
  i.toString(16).padStart(2, "0")
);

export async function sha256Bytes(data: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data);
  return toHex(new Uint8Array(digest));
}

export async function sha256Text(text: string): Promise<string> {
  return sha256Bytes(TEXT_ENCODER.encode(text).buffer);
}

export async function sha256TextWithLength(
  text: string
): Promise<{ hash: string; byteLength: number }> {
  const bytes = TEXT_ENCODER.encode(text);
  const hash = await sha256Bytes(bytes.buffer);
  return { hash, byteLength: bytes.byteLength };
}

function toHex(bytes: Uint8Array): string {
  const parts = new Array<string>(bytes.length);
  for (let i = 0; i < bytes.length; i++) {
    parts[i] = HEX_BYTES[bytes[i]];
  }
  return parts.join("");
}
