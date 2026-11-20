export const connectionNameLimit = 32;

export function ipv4InputValid(value: string): boolean {
  const parts = value.split(".");
  return parts.length === 4 && parts.every((part) => /^(?:0|[1-9]\d{0,2})$/.test(part) && Number(part) <= 255);
}
export function ipv6InputValid(value: string): boolean {
  if (!value.includes(":")) return false;
  try {
    return new URL(`http://[${value}]/`).hostname.length > 2;
  } catch {
    return false;
  }
}
export function hostInputValid(value: string): boolean {
  if (!value || /[\s\u0000-\u001f]/.test(value)) return false;
  if (ipv4InputValid(value) || ipv6InputValid(value)) return true;
  if (value.includes(":") || /^[\d.]+$/.test(value)) return false;
  const hostname = value.endsWith(".") ? value.slice(0, -1) : value;
  return hostname.length > 0 && hostname.length <= 253 && hostname.split(".").every((label) => (
    label.length > 0
    && label.length <= 63
    && /^[A-Za-z0-9](?:[A-Za-z0-9_-]*[A-Za-z0-9])?$/.test(label)
  ));
}
export function destinationInputValid(value: string): boolean {
  const destination = value.trim();
  if (!destination || destination.length > 320) return false;
  const parts = destination.split("@");
  if (parts.length > 2) return false;
  const hostInput = parts.length === 2 ? parts[1] : parts[0];
  if (parts.length === 2 && !/^[A-Za-z0-9][A-Za-z0-9._-]{0,252}$/.test(parts[0])) return false;
  const bracketed = hostInput.startsWith("[") || hostInput.endsWith("]");
  if (bracketed) {
    return hostInput.startsWith("[") && hostInput.endsWith("]") && ipv6InputValid(hostInput.slice(1, -1));
  }
  return hostInputValid(hostInput);
}
export function identityPathInputValid(value: string | null): boolean {
  const path = value?.trim() ?? "";
  if (!path) return false;
  if (/^(?:\\\\[?.]\\|\/\/[?.]\/)/.test(path) || /[<>"|?*\u0000-\u001f]/.test(path)) return false;
  return ![...path].some((character, index) => character === ":" && index !== 1);
}
export function displayNameInputValid(value: string): boolean {
  const name = value.trim();
  return name.length > 0 && [...name].length <= connectionNameLimit && !/[\u0000-\u001f\u007f-\u009f]/.test(name);
}
