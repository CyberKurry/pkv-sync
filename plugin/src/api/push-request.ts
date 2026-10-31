import type { PushChange } from "../sync/types";
import { textByteLength } from "../sync/text-encoding";

export function makePushBody(changes: PushChange[], deviceName: string) {
  return { changes, device_name: deviceName };
}

export function pushBodyBytes(changes: PushChange[], deviceName: string): number {
  return textByteLength(JSON.stringify(makePushBody(changes, deviceName)));
}

export function pushChangeBytes(change: PushChange): number {
  return textByteLength(JSON.stringify(change));
}
