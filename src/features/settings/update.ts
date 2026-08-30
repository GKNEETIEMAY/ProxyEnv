export type UpdateState = "idle" | "checking" | "latest" | "available" | "unpublished" | "error";

export type ReleaseNoteLine = {
  kind: "heading" | "item" | "paragraph";
  text: string;
};

export type GitHubRelease = {
  tag_name?: string;
  html_url?: string;
  body?: string | null;
  published_at?: string | null;
};

export function compareVersions(left: string, right: string): number {
  const normalize = (value: string) => value
    .replace(/^v/i, "")
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);
  const leftParts = normalize(left);
  const rightParts = normalize(right);
  const length = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

export function parseReleaseNotes(body: string | null | undefined): ReleaseNoteLine[] {
  if (!body) return [];
  return body
    .slice(0, 20_000)
    .split(/\r?\n/)
    .map((rawLine) => rawLine.trim().slice(0, 600))
    .filter((line) => line.length > 0 && !/^(?:[*_]{1,2})?full\s+changelog(?:[*_]{1,2})?\s*:/i.test(line))
    .map((line): ReleaseNoteLine => {
      const heading = /^#{1,6}\s+/.test(line);
      const item = /^(?:[-*+]\s+|\d+\.\s+)/.test(line);
      const text = line
        .replace(/^#{1,6}\s+/, "")
        .replace(/^(?:[-*+]\s+|\d+\.\s+)/, "")
        .replace(/\[([^\]]+)]\(https?:\/\/[^)]+\)/g, "$1")
        .replace(/<[^>]*>/g, "")
        .replace(/[`*_~]/g, "")
        .trim();
      return { kind: heading ? "heading" : item ? "item" : "paragraph", text };
    })
    .filter((line) => line.text.length > 0)
    .slice(0, 32);
}

export function isOfficialReleaseUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "https:"
      && url.hostname === "github.com"
      && url.pathname.startsWith("/GKNEETIEMAY/ProxyEnv/releases/");
  } catch {
    return false;
  }
}
