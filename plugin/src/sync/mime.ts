import { extensionOf } from "../util";

const MIME_BY_EXTENSION: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  pdf: "application/pdf",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  mp4: "video/mp4"
};

export function guessMime(path: string): string | undefined {
  const ext = extensionOf(path);
  return ext ? MIME_BY_EXTENSION[ext] : undefined;
}
