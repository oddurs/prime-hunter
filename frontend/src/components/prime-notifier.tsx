"use client";

import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { useWs } from "@/contexts/websocket-context";

export function NotificationToaster() {
  const { notifications } = useWs();
  const shownIds = useRef(new Set<number>());

  useEffect(() => {
    for (const notif of notifications) {
      if (shownIds.current.has(notif.id)) continue;
      shownIds.current.add(notif.id);

      const description =
        notif.details.length > 0 ? notif.details.join(", ") : undefined;

      switch (notif.kind) {
        case "prime":
          toast.success(notif.title, { description, duration: 8000 });
          break;
        case "error":
          toast.error(notif.title, { description, duration: 10000 });
          break;
        case "search_start":
        case "search_done":
        case "milestone":
          toast.info(notif.title, { description, duration: 6000 });
          break;
        default:
          toast(notif.title, { description });
      }
    }
  }, [notifications]);

  return null;
}
