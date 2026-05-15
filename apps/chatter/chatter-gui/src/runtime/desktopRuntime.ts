import type { ExportFormat } from "../protocol/desktopProtocol";
import { createClanCapability } from "./capabilities/clan";
import type {
  ClanCapability,
  DesktopEnvironmentCapability,
  DesktopRuntime,
  DesktopTransport,
  ExportCapability,
  OpenInClanRequest,
  ValidationDragDropEvent,
  ValidationExportEntry,
  ValidationRun,
  ValidationRunnerCapability,
  ValidationTargetCapability,
} from "./capabilities/contracts";
import { createDesktopEnvironmentCapability } from "./capabilities/environment";
import { createExportCapability } from "./capabilities/exportResults";
import { createValidationRunnerCapability } from "./capabilities/validationRunner";
import { createValidationTargetCapability } from "./capabilities/validationTarget";
import { tauriTransport } from "./tauriTransport";

export function createDesktopRuntime(
  transport: DesktopTransport = tauriTransport,
): DesktopRuntime {
  return {
    environment: createDesktopEnvironmentCapability(transport),
    validationRunner: createValidationRunnerCapability(transport),
    validationTarget: createValidationTargetCapability(transport),
    clan: createClanCapability(transport),
    exports: createExportCapability(transport),
  };
}

export const desktopRuntime = createDesktopRuntime();

export type {
  ClanCapability,
  DesktopEnvironmentCapability,
  DesktopRuntime,
  ExportCapability,
  OpenInClanRequest,
  ValidationDragDropEvent,
  ValidationExportEntry,
  ValidationRun,
  ValidationRunnerCapability,
  ValidationTargetCapability,
};
export type { ExportFormat };
