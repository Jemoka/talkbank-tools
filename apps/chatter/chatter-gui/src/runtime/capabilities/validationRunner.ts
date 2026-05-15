import { DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import type { DesktopTransport, ValidationRunnerCapability } from "./contracts";
import { disposeOnce } from "./shared";

export function createValidationRunnerCapability(
  transport: Pick<DesktopTransport, "invoke" | "listenValidationEvent">,
): ValidationRunnerCapability {
  return {
    async startValidation(path, onEvent) {
      const unlisten = disposeOnce(
        await transport.listenValidationEvent((event) => {
          onEvent(event);
        }),
      );

      try {
        await transport.invoke(DESKTOP_COMMANDS.validate, { path });
      } catch (error) {
        unlisten();
        throw error;
      }

      return {
        cancel: async () => {
          await transport.invoke(DESKTOP_COMMANDS.cancelValidation);
        },
        dispose: unlisten,
      };
    },
  };
}
