import type { DesktopTransport, ValidationTargetCapability } from "./contracts";
import { disposeOnce, singlePathSelection } from "./shared";

export function createValidationTargetCapability(
  transport: Pick<
    DesktopTransport,
    "chooseValidationFile" | "chooseValidationFolder" | "onValidationDragDrop"
  >,
): ValidationTargetCapability {
  return {
    async chooseValidationFile() {
      return singlePathSelection(await transport.chooseValidationFile());
    },

    async chooseValidationFolder() {
      return singlePathSelection(await transport.chooseValidationFolder());
    },

    async onValidationDragDrop(listener) {
      return disposeOnce(await transport.onValidationDragDrop(listener));
    },
  };
}
