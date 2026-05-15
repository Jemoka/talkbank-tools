import type {
  DesktopEnvironmentCapability,
  DesktopTransport,
} from "./contracts";

export function createDesktopEnvironmentCapability(
  transport: Pick<DesktopTransport, "isNativeDesktop">,
): DesktopEnvironmentCapability {
  return {
    isNativeDesktop() {
      return transport.isNativeDesktop();
    },
  };
}
