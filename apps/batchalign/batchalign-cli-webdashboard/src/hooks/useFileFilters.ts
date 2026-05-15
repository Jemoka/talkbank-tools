import { useMemo, useState } from "react";
import type { FileStatusEntry } from "../types";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type FilterTab = "all" | "error" | "processing" | "done" | "queued";

export type ErrorCodeGroup = {
  code: string;       // e.g. "E705" or "general"
  label: string;      // first line of error message
  files: FileStatusEntry[];
};

export type ErrorGroup = {
  category: string;        // "input" | "media" | "system" | "processing"
  categoryLabel: string;   // "CHAT Parse Error" etc.
  codeGroups: ErrorCodeGroup[];
  totalFiles: number;
};

export type FileCounts = {
  all: number;
  error: number;
  processing: number;
  done: number;
  queued: number;
};

const PAGE_SIZE = 50;

const STATUS_ORDER: Record<string, number> = {
  error: 0,
  processing: 1,
  done: 2,
  queued: 3,
};

/**
 * Maps backend `FailureCategory` wire values to display-friendly group names.
 *
 * Backend categories (from Rust `FailureCategory` enum) are kebab-cased:
 *   validation, parse_error, input_missing, worker_crash, worker_timeout,
 *   worker_protocol, provider_transient, provider_terminal, memory_pressure,
 *   cancelled, system.
 *
 * We collapse these into 5 user-facing groups:
 *   input, media, system, processing, validation.
 */
const CATEGORY_NORMALIZE: Record<string, string> = {
  validation: "validation",
  parse_error: "input",
  input_missing: "media",
  worker_crash: "system",
  worker_timeout: "system",
  worker_protocol: "system",
  provider_transient: "processing",
  provider_terminal: "processing",
  memory_pressure: "system",
  cancelled: "system",
  system: "system",
  // Legacy/fallback values from older display groups
  input: "input",
  media: "media",
  processing: "processing",
};

const CATEGORY_DISPLAY: Record<string, string> = {
  input: "CHAT Parse Error",
  media: "Media Not Found",
  system: "System Error",
  processing: "Processing Error",
  validation: "Pipeline Bug",
};

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useFileFilters(files: FileStatusEntry[]) {
  const [activeTab, setActiveTab] = useState<FilterTab>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [page, setPage] = useState(1);

  // Counts per status
  const counts: FileCounts = useMemo(() => {
    let error = 0, processing = 0, done = 0, queued = 0;
    for (const f of files) {
      if (f.status === "error") error++;
      else if (f.status === "processing") processing++;
      else if (f.status === "done") done++;
      else if (f.status === "queued") queued++;
    }
    return { all: files.length, error, processing, done, queued };
  }, [files]);

  // Error groups: category -> code -> files
  const errorGroups: ErrorGroup[] = useMemo(() => {
    const errorFiles = files.filter((f) => f.status === "error");
    if (errorFiles.length === 0) return [];

    // Group by normalized display category. Backend sends fine-grained
    // FailureCategory values (worker_crash, provider_transient, etc.);
    // we collapse them into user-friendly groups.
    const catMap = new Map<string, FileStatusEntry[]>();
    for (const f of errorFiles) {
      const rawCat = f.error_category ?? "processing";
      const cat = CATEGORY_NORMALIZE[rawCat] ?? "processing";
      const list = catMap.get(cat);
      if (list) list.push(f);
      else catMap.set(cat, [f]);
    }

    const groups: ErrorGroup[] = [];
    for (const [cat, catFiles] of catMap) {
      // Group by individual error code within category
      const codeMap = new Map<string, FileStatusEntry[]>();
      for (const f of catFiles) {
        const codes = f.error_codes;
        if (codes && codes.length > 0) {
          for (const code of codes) {
            const list = codeMap.get(code);
            if (list) list.push(f);
            else codeMap.set(code, [f]);
          }
        } else {
          const list = codeMap.get("general");
          if (list) list.push(f);
          else codeMap.set("general", [f]);
        }
      }

      const codeGroups: ErrorCodeGroup[] = [];
      for (const [code, codeFiles] of codeMap) {
        // Use first line of the first file's error as the label
        const firstError = codeFiles[0]?.error ?? "Unknown error";
        const label = firstError.split("\n")[0];
        codeGroups.push({ code, label, files: codeFiles });
      }
      // Sort: specific codes first (alphabetically), "general" last
      codeGroups.sort((a, b) => {
        if (a.code === "general") return 1;
        if (b.code === "general") return -1;
        return a.code.localeCompare(b.code);
      });

      groups.push({
        category: cat,
        categoryLabel: CATEGORY_DISPLAY[cat] ?? cat,
        codeGroups,
        totalFiles: catFiles.length,
      });
    }

    // Sort categories: validation first (pipeline bugs), then input, media, processing, system
    const catOrder: Record<string, number> = { validation: 0, input: 1, media: 2, processing: 3, system: 4 };
    groups.sort((a, b) => (catOrder[a.category] ?? 99) - (catOrder[b.category] ?? 99));
    return groups;
  }, [files]);

  // Filtered + sorted files
  const filteredFiles = useMemo(() => {
    let result = files;

    // Tab filter
    if (activeTab !== "all") {
      result = result.filter((f) => f.status === activeTab);
    }

    // Search filter (by filename)
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter((f) => f.filename.toLowerCase().includes(q));
    }

    // Sort: errors first, then processing, done, queued; alphabetical within
    return [...result].sort(
      (a, b) =>
        (STATUS_ORDER[a.status] ?? 99) - (STATUS_ORDER[b.status] ?? 99) ||
        a.filename.localeCompare(b.filename),
    );
  }, [files, activeTab, searchQuery]);

  // Pagination
  const totalPages = Math.max(1, Math.ceil(filteredFiles.length / PAGE_SIZE));

  // Clamp page when filters change
  const clampedPage = Math.min(page, totalPages);
  if (clampedPage !== page) {
    // Schedule state update for next render
    queueMicrotask(() => setPage(clampedPage));
  }

  const pageFiles = filteredFiles.slice(
    (clampedPage - 1) * PAGE_SIZE,
    clampedPage * PAGE_SIZE,
  );

  return {
    // State
    activeTab,
    setActiveTab: (tab: FilterTab) => { setActiveTab(tab); setPage(1); },
    searchQuery,
    setSearchQuery: (q: string) => { setSearchQuery(q); setPage(1); },
    page: clampedPage,
    setPage,
    // Derived
    counts,
    errorGroups,
    filteredFiles,
    pageFiles,
    totalPages,
    pageSize: PAGE_SIZE,
  };
}
