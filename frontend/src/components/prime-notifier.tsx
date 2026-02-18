"use client";

/**
 * @module prime-notifier
 *
 * Invisible component that watches for new prime discoveries via the
 * WebSocket notification stream and displays toast notifications
 * (via Sonner) and browser notifications (via the Notification API).
 * Mounted once in the root layout — no visible UI.
 *
 * @see {@link src/hooks/use-notifications.ts} — browser notification hook
 */

import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { useWs } from "@/contexts/websocket-context";
import { useBrowserNotifications } from "@/hooks/use-notifications";

export function NotificationToaster() {
  const { notifications } = useWs();
  const { show } = useBrowserNotifications();
  const seenIds = useRef(new Set<number>());
  const initialized = useRef(false);

  useEffect(() => {
    if (!notifications.length) return;

    // First update: mark all existing notifications as seen without toasting
    if (!initialized.current) {
      for (const notif of notifications) {
        seenIds.current.add(notif.id);
      }
      initialized.current = true;
      return;
    }

    for (const notif of notifications) {
      if (seenIds.current.has(notif.id)) continue;
      seenIds.current.add(notif.id);

      const description =
        notif.details.length > 0 ? notif.details.join(", ") : undefined;

      switch (notif.kind) {
        case "prime":
          toast.success(notif.title, { description, duration: 8000 });
          show(notif.title, { body: description, tag: `prime-${notif.id}` });
          break;
        case "error":
          toast.error(notif.title, { description, duration: 10000 });
          break;
        case "search_start":
        case "search_done":
        case "milestone":
          toast.info(notif.title, { description, duration: 6000 });
          if (notif.kind === "search_done") {
            show(notif.title, { body: description, tag: `done-${notif.id}` });
          }
          break;
        default:
          toast(notif.title, { description });
      }
    }
  }, [notifications, show]);

  return null;
}
