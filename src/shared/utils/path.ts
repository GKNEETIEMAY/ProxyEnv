export function withoutWindowsExtendedPathPrefix(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) return `\\\\${path.slice(8)}`;
  if (path.startsWith("\\\\?\\") || path.startsWith("\\??\\")) return path.slice(4);
  return path;
}
