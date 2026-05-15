/** Mirrors Rust `Severity` enum from talkbank-model */
export type Severity = "Error" | "Warning";

/** Mirrors Rust `Span` struct */
export interface Span {
  start: number;
  end: number;
}

/** Mirrors Rust `SourceLocation` */
export interface SourceLocation {
  start: number;
  end: number;
  line?: number;
  column?: number;
}

/** Mirrors Rust `ErrorLabel` */
export interface ErrorLabel {
  start: number;
  end: number;
  message: string;
}

/** Mirrors Rust `ParseError` (serialized form) */
export interface ParseError {
  code: string;
  severity: Severity;
  location: SourceLocation;
  labels: ErrorLabel[];
  message: string;
  suggestion?: string;
  help_url?: string;
}

/** File validation status — mirrors Rust `FrontendFileStatus` */
export type FileStatus =
  | { type: "valid"; cacheHit: boolean }
  | { type: "invalid"; errorCount: number; cacheHit: boolean }
  | { type: "roundtripFailed"; cacheHit: boolean; reason: string }
  | { type: "parseError"; message: string }
  | { type: "readError"; message: string };

/** Mirrors Rust `FrontendStats` */
export interface ValidationStats {
  totalFiles: number;
  validFiles: number;
  invalidFiles: number;
  cacheHits: number;
  cacheMisses: number;
  parseErrors: number;
  roundtripPassed: number;
  roundtripFailed: number;
  cancelled: boolean;
}

/** A parse diagnostic paired with pre-rendered miette HTML from Rust */
export interface RenderedDiagnostic {
  error: ParseError;
  renderedHtml: string;
  /** Plain text rendering (no ANSI) for clipboard copy */
  renderedText: string;
}

/** Events emitted from the Rust backend via Tauri's event bridge */
export type ValidationEvent =
  | { type: "discovering" }
  | { type: "started"; totalFiles: number }
  | {
      type: "errors";
      file: string;
      diagnostics: RenderedDiagnostic[];
      source: string;
    }
  | { type: "fileComplete"; file: string; status: FileStatus }
  | { type: "finished"; stats: ValidationStats };

/** Per-file state accumulated from the event stream */
export interface FileEntry {
  path: string;
  /** Display name (relative to the validated root) */
  name: string;
  diagnostics: RenderedDiagnostic[];
  source: string;
  status: FileStatus | null;
}

/** Tree node for the collapsible file tree */
export interface TreeNode {
  name: string;
  path: string;
  children: TreeNode[];
  file: FileEntry | null;
}
