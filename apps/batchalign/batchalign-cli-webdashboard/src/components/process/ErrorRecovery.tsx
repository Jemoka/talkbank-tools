/** Structured error display with suggested recovery actions.
 *
 * Maps server error categories to user-friendly messages and actionable
 * suggestions. Shown in the processing progress view when files fail.
 * Replaces raw server error strings with context-appropriate guidance
 * for nontechnical users.
 */

import type { FileStatusEntry } from "../../types";
import type { components } from "../../generated/api";

type FailureCategory = components["schemas"]["FailureCategory"];

interface ErrorInfo {
  /** User-friendly title for the error category. */
  title: string;
  /** Plain-language explanation of what went wrong. */
  explanation: string;
  /** Suggested actions the user can take. */
  suggestions: string[];
}

/** Map server failure categories to user-friendly error info. */
function categorize(category: FailureCategory | null | undefined): ErrorInfo {
  switch (category) {
    case "validation":
      return {
        title: "File validation error",
        explanation:
          "Some files have formatting issues that prevent processing.",
        suggestions: [
          "Open the file in a text editor and check for syntax errors",
          "Make sure the file is in valid CHAT format",
          "Try running the file through a CHAT validator first",
        ],
      };
    case "parse_error":
      return {
        title: "File could not be read",
        explanation: "The file format was not recognized or is corrupted.",
        suggestions: [
          "Make sure the file is a valid .cha file (not renamed from another format)",
          "Check that the file encoding is UTF-8",
          "Try opening the file in a text editor to verify its contents",
        ],
      };
    case "input_missing":
      return {
        title: "File not found",
        explanation:
          "The input file could not be found at the expected location.",
        suggestions: [
          "Make sure the file hasn't been moved or deleted",
          "Check that the file path is correct",
          "Try selecting the folder again",
        ],
      };
    case "worker_crash":
      return {
        title: "Processing engine crashed",
        explanation:
          "The ML model encountered an unexpected error while processing this file.",
        suggestions: [
          "Try processing the file again — transient crashes often resolve on retry",
          "If it keeps failing, the file may have unusual content that triggers a bug",
          "Check that your machine has enough free memory",
        ],
      };
    case "worker_timeout":
      return {
        title: "Processing timed out",
        explanation:
          "The file took too long to process and was stopped.",
        suggestions: [
          "Large audio files may need more time — try again with fewer files",
          "Make sure your machine isn't running low on memory or CPU",
          "For very long recordings, consider splitting them into shorter segments",
        ],
      };
    case "worker_protocol":
      return {
        title: "Internal communication error",
        explanation:
          "The processing engine sent an unexpected response.",
        suggestions: [
          "Try restarting the server and processing again",
          "This is likely a bug — please report it if it persists",
        ],
      };
    case "provider_transient":
      return {
        title: "Temporary service error",
        explanation:
          "The cloud ASR service (Rev.AI) returned a temporary error.",
        suggestions: [
          "Wait a moment and try again — the service may be briefly overloaded",
          "Check your internet connection",
          "If using Rev.AI, verify your API key is still valid",
        ],
      };
    case "provider_terminal":
      return {
        title: "Service rejected the request",
        explanation:
          "The cloud ASR service returned a permanent error for this file.",
        suggestions: [
          "Check that your Rev.AI API key is valid and has remaining credit",
          "The audio file may be in an unsupported format",
          "Try switching to Whisper (local) as the ASR engine",
        ],
      };
    case "memory_pressure":
      return {
        title: "Not enough memory",
        explanation:
          "The server ran out of available memory and had to stop processing.",
        suggestions: [
          "Close other applications to free up memory",
          "Try processing fewer files at a time",
          "Restart the server and try again",
        ],
      };
    case "cancelled":
      return {
        title: "Processing was cancelled",
        explanation: "This file was cancelled before it finished processing.",
        suggestions: ["You can restart the job to try again"],
      };
    case "system":
    default:
      return {
        title: "Unexpected error",
        explanation: "Something went wrong that we didn't anticipate.",
        suggestions: [
          "Try processing the file again",
          "If the error persists, restart the server",
          "Check the server logs for more details",
        ],
      };
  }
}

interface ErrorRecoveryProps {
  /** Files that have errors. */
  errorFiles: FileStatusEntry[];
}

export function ErrorRecovery({ errorFiles }: ErrorRecoveryProps) {
  if (errorFiles.length === 0) return null;

  // Group errors by category for a cleaner display
  const byCategory = new Map<string, FileStatusEntry[]>();
  for (const f of errorFiles) {
    const key = f.error_category ?? "system";
    const group = byCategory.get(key) ?? [];
    group.push(f);
    byCategory.set(key, group);
  }

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-semibold text-red-700">
        {errorFiles.length} file{errorFiles.length !== 1 ? "s" : ""} had errors
      </h3>

      {[...byCategory.entries()].map(([category, files]) => {
        const info = categorize(category as FailureCategory);
        return (
          <div
            key={category}
            className="bg-red-50 border border-red-200 rounded-lg p-4"
          >
            <div className="text-sm font-medium text-red-800">
              {info.title}
            </div>
            <p className="text-xs text-red-600 mt-1">{info.explanation}</p>

            {/* Affected files */}
            <div className="mt-2 space-y-1">
              {files.map((f) => (
                <div key={f.filename} className="text-xs text-red-700">
                  <span className="font-mono">{f.filename}</span>
                  {f.error && (
                    <span className="text-red-500 ml-1">— {f.error}</span>
                  )}
                  {f.error_codes && f.error_codes.length > 0 && (
                    <span className="text-red-400 ml-1">
                      ({f.error_codes.join(", ")})
                    </span>
                  )}
                </div>
              ))}
            </div>

            {/* Suggestions */}
            <div className="mt-3 border-t border-red-200 pt-2">
              <div className="text-xs font-medium text-red-700 mb-1">
                What to try:
              </div>
              <ul className="text-xs text-red-600 space-y-0.5 list-disc list-inside">
                {info.suggestions.map((s) => (
                  <li key={s}>{s}</li>
                ))}
              </ul>
            </div>
          </div>
        );
      })}
    </div>
  );
}
