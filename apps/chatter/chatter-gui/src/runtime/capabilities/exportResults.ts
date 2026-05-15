import { DESKTOP_COMMANDS } from "../../protocol/desktopProtocol";
import type { DesktopTransport, ExportCapability } from "./contracts";

export function createExportCapability(
  transport: Pick<DesktopTransport, "chooseExportPath" | "invoke">,
): ExportCapability {
  return {
    async chooseExportPath() {
      return transport.chooseExportPath();
    },

    async exportResults(results, format, path) {
      await transport.invoke(DESKTOP_COMMANDS.exportResults, {
        results: JSON.stringify(results),
        format,
        path,
      });
    },
  };
}
