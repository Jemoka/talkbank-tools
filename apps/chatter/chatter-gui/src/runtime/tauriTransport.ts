import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  DESKTOP_EVENTS,
  type DesktopCommandArgs,
  type DesktopCommandName,
  type DesktopCommandResultMap,
  type ValidationEventPayload,
} from "../protocol/desktopProtocol";
import type { DesktopTransport } from "./capabilities/contracts";

function isNativeDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const tauriTransport: DesktopTransport = {
  isNativeDesktop,

  invoke<C extends DesktopCommandName>(
    command: C,
    ...args: DesktopCommandArgs<C>
  ): Promise<DesktopCommandResultMap[C]> {
    const payload = args[0] as Record<string, unknown> | undefined;
    return invoke<DesktopCommandResultMap[C]>(command, payload);
  },

  async listenValidationEvent(listener) {
    const unlisten = await listen<ValidationEventPayload>(
      DESKTOP_EVENTS.validation,
      (event) => {
        listener(event.payload);
      },
    );

    return () => {
      void unlisten();
    };
  },

  async chooseValidationFile() {
    return open({
      multiple: false,
      filters: [{ name: "CHAT files", extensions: ["cha"] }],
    });
  },

  async chooseValidationFolder() {
    return open({
      directory: true,
      multiple: false,
    });
  },

  async chooseExportPath() {
    return save({
      filters: [
        { name: "JSON", extensions: ["json"] },
        { name: "Text", extensions: ["txt"] },
      ],
    });
  },

  async onValidationDragDrop(listener) {
    if (!isNativeDesktop()) {
      return () => {};
    }

    const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      switch (event.payload.type) {
        case "enter":
        case "over":
        case "leave":
          listener({ type: event.payload.type });
          break;
        case "drop":
          listener({ type: "drop", paths: event.payload.paths });
          break;
      }
    });

    return () => {
      void unlisten();
    };
  },
};
