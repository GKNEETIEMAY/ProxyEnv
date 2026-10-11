import { reactive, watch, type WatchSource } from "vue";

export type NotificationKind = "success" | "error" | "warning" | "info";
export interface Notification {
  id: number;
  key: string;
  kind: NotificationKind;
  message: string;
  title?: string;
}

// Session-only feedback. Never persist, upload or log notification contents.
export const notifications = reactive<Notification[]>([]);
let sequence = 0;
const timers = new Map<number, { timer?: ReturnType<typeof setTimeout>; remaining: number; started: number }>();

export function dismissNotification(id: number) {
  clearTimeout(timers.get(id)?.timer);
  timers.delete(id);
  const index = notifications.findIndex(item => item.id === id);
  if (index >= 0) notifications.splice(index, 1);
}

export function pauseNotification(id: number) {
  const entry = timers.get(id);
  if (!entry || entry.timer === undefined) return;
  clearTimeout(entry.timer);
  entry.timer = undefined;
  entry.remaining = Math.max(0, entry.remaining - (Date.now() - entry.started));
}

export function resumeNotification(id: number) {
  const entry = timers.get(id);
  if (!entry || entry.timer !== undefined) return;
  entry.started = Date.now();
  entry.timer = setTimeout(() => dismissNotification(id), entry.remaining);
}

export function notify(message: string, kind: NotificationKind = "info", key?: string, title?: string) {
  if (!message.trim()) return;
  // A repeated error updates one notice rather than flooding the desktop.
  const identity = key ?? `${kind}:${message}`;
  const existing = notifications.find(item => item.key === identity);
  const paused = existing && timers.get(existing.id)?.timer === undefined;
  const id = existing?.id ?? ++sequence;
  const notice = { id, key: identity, kind, message, title };
  if (existing) {
    Object.assign(existing, notice);
    clearTimeout(timers.get(id)?.timer);
  } else {
    while (notifications.length >= 3) dismissNotification(notifications[0].id);
    notifications.push(notice);
  }
  timers.set(id, { remaining: kind === "error" || kind === "warning" ? 10000 : 4000, started: Date.now() });
  if (!paused) resumeNotification(id);
  return id;
}

export function clearNotifications() {
  for (const notice of [...notifications]) dismissNotification(notice.id);
}

// Existing localized operation state can migrate without a second error formatter.
export function useNotification(source: WatchSource<string | undefined>, kind: NotificationKind, key: string) {
  return watch(source, message => { if (message) notify(message, kind, key); });
}
