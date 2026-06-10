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

function toHex(bytes: Uint8Array): string {
  let hex = "";
  for (const byte of bytes) {
    hex += HEX_BYTES[byte];
  }
  return hex;
}
