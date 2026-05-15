import type { FileEntry, ValidationEvent, ValidationStats } from "../protocol/validation";

export type Phase = "idle" | "discovering" | "running" | "finished";

export interface ValidationState {
  phase: Phase;
  files: Map<string, FileEntry>;
  totalFiles: number;
  processedFiles: number;
  totalErrors: number;
  stats: ValidationStats | null;
}

export function createInitialValidationState(): ValidationState {
  return {
    phase: "idle",
    files: new Map(),
    totalFiles: 0,
    processedFiles: 0,
    totalErrors: 0,
    stats: null,
  };
}

export function applyValidationEvent(
  prev: ValidationState,
  event: ValidationEvent,
  relativeName: (path: string) => string,
): ValidationState {
  switch (event.type) {
    case "discovering":
      return { ...prev, phase: "discovering" };

    case "started":
      return { ...prev, phase: "running", totalFiles: event.totalFiles };

    case "errors": {
      const files = new Map(prev.files);
      const existing = files.get(event.file);

      files.set(
        event.file,
        existing
          ? {
              ...existing,
              diagnostics: [...existing.diagnostics, ...event.diagnostics],
              source: event.source,
            }
          : {
              path: event.file,
              name: relativeName(event.file),
              diagnostics: [...event.diagnostics],
              source: event.source,
              status: null,
            },
      );

      return {
        ...prev,
        files,
        totalErrors: prev.totalErrors + event.diagnostics.length,
      };
    }

    case "fileComplete": {
      const files = new Map(prev.files);
      const existing = files.get(event.file);

      files.set(
        event.file,
        existing
          ? { ...existing, status: event.status }
          : {
              path: event.file,
              name: relativeName(event.file),
              diagnostics: [],
              source: "",
              status: event.status,
            },
      );

      return { ...prev, files, processedFiles: prev.processedFiles + 1 };
    }

    case "finished":
      return { ...prev, phase: "finished", stats: event.stats };
  }

  return assertNever(event);
}

export function relativeDisplayName(fullPath: string, targetPath: string): string {
  if (!targetPath) return normalizeDisplayPath(fullPath);
  if (fullPath === targetPath) return basename(fullPath);

  const targetWithSeparator = withTrailingSeparator(targetPath);
  if (fullPath.startsWith(targetWithSeparator)) {
    return normalizeDisplayPath(fullPath.slice(targetWithSeparator.length));
  }

  return normalizeDisplayPath(fullPath);
}

function normalizeDisplayPath(path: string): string {
  return path.replace(/\\/g, "/");
}

function basename(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

function withTrailingSeparator(path: string): string {
  if (path === "" || /[\\/]$/.test(path)) return path;
  const separator = path.includes("\\") ? "\\" : "/";
  return `${path}${separator}`;
}

function assertNever(value: never): never {
  throw new Error(`Unhandled validation event: ${JSON.stringify(value)}`);
}
