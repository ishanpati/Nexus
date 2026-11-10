import { useCallback, useEffect, useState, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";

export function useLauncherWindow(inputRef: RefObject<HTMLInputElement | null>) {
  const [windowError, setWindowError] = useState("");
  const [shortcutNotice, setShortcutNotice] = useState("");
  const startDrag = useCallback(async () => {
    try {
      await invoke("start_launcher_drag");
      setWindowError("");
    } catch (error) {
      setWindowError(String(error));
    }
  }, []);
  const hideWindow = useCallback(async () => {
    try {
      await invoke("hide_launcher");
      setWindowError("");
    } catch (error) {
      setWindowError("Could not hide NEXUS: " + String(error));
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const trackListener = (listener: Promise<UnlistenFn>) => {
      void listener.then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      }).catch((error: unknown) => {
        if (!disposed) setWindowError("Could not track the NEXUS window: " + String(error));
      });
    };
    const focusInput = () => inputRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.isComposing) {
        event.preventDefault();
        void hideWindow();
      }
    };

    focusInput();
    window.addEventListener("focus", focusInput);
    window.addEventListener("keydown", onKeyDown);
    const appWindow = getCurrentWindow();
    trackListener(appWindow.onFocusChanged(({ payload: focused }) => {
      if (focused && !disposed) focusInput();
    }));
    trackListener(appWindow.listen("launcher-shown", () => {
      if (!disposed) focusInput();
    }));
    trackListener(appWindow.listen<string>("launcher-error", ({ payload }) => {
      if (!disposed) setWindowError(payload);
    }));
    void invoke<{ registered: boolean; warning: string | null }>("shortcut_status").then((status) => {
      if (!disposed) setShortcutNotice(status.warning ?? "");
    }).catch((error: unknown) => {
      if (!disposed) setWindowError("Could not read shortcut status: " + String(error));
    });

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener("focus", focusInput);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [hideWindow, inputRef]);

  return { hideWindow, startDrag, windowError, shortcutNotice };
}
