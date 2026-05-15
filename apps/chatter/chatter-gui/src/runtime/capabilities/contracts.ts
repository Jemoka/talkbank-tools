import type {
  DesktopCommandArgs,
  DesktopCommandName,
  DesktopCommandResultMap,
  ExportFormat,
  ValidationEventPayload,
} from "../../protocol/desktopProtocol";
import type { FileStatus, ParseError, ValidationEvent } from "../../protocol/validation";

export interface ValidationExportEntry {
  path: string;
  errors: ParseError[];
  status: FileStatus | null;
}

export interface OpenInClanRequest {
  file: string;
  error: ParseError;
}

export type ValidationDragDropEvent =
  | { type: "enter" | "over" | "leave" }
  | { type: "drop"; paths: string[] };

export interface ValidationRun {
  cancel(): Promise<void>;
  dispose(): void;
}

export interface DesktopEnvironmentCapability {
  isNativeDesktop(): boolean;
}

export interface ValidationRunnerCapability {
  startValidation(
    path: string,
    onEvent: (event: ValidationEvent) => void,
  ): Promise<ValidationRun>;
}

export interface ValidationTargetCapability {
  chooseValidationFile(): Promise<string | null>;
  chooseValidationFolder(): Promise<string | null>;
  onValidationDragDrop(
    listener: (event: ValidationDragDropEvent) => void,
  ): Promise<() => void>;
}

export interface ClanCapability {
  checkClanAvailable(): Promise<boolean>;
  openInClan(request: OpenInClanRequest): Promise<void>;
}

export interface ExportCapability {
  chooseExportPath(): Promise<string | null>;
  exportResults(
    results: ValidationExportEntry[],
    format: ExportFormat,
    path: string,
  ): Promise<void>;
}

export interface DesktopRuntime {
  environment: DesktopEnvironmentCapability;
  validationRunner: ValidationRunnerCapability;
  validationTarget: ValidationTargetCapability;
  clan: ClanCapability;
  exports: ExportCapability;
}

export interface DesktopTransport {
  isNativeDesktop(): boolean;
  invoke<C extends DesktopCommandName>(
    command: C,
    ...args: DesktopCommandArgs<C>
  ): Promise<DesktopCommandResultMap[C]>;
  listenValidationEvent(
    listener: (event: ValidationEventPayload) => void,
  ): Promise<() => void>;
  chooseValidationFile(): Promise<string | string[] | null>;
  chooseValidationFolder(): Promise<string | string[] | null>;
  chooseExportPath(): Promise<string | null>;
  onValidationDragDrop(
    listener: (event: ValidationDragDropEvent) => void,
  ): Promise<() => void>;
}
