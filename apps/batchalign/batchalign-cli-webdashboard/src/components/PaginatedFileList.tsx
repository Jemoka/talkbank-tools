import { FileTable } from "./FileTable";
import type { FileStatusEntry } from "../types";

function PageControls({
  page,
  totalPages,
  onPageChange,
}: {
  page: number;
  totalPages: number;
  onPageChange: (p: number) => void;
}) {
  if (totalPages <= 1) return null;

  return (
    <div className="flex items-center justify-end gap-2 text-xs">
      <button
        type="button"
        disabled={page <= 1}
        className="px-2 py-1 rounded border border-zinc-200 text-zinc-600 hover:bg-zinc-50 disabled:opacity-30 disabled:cursor-default cursor-pointer"
        onClick={() => onPageChange(page - 1)}
      >
        Prev
      </button>
      <span className="text-zinc-500 font-mono">
        {page} / {totalPages}
      </span>
      <button
        type="button"
        disabled={page >= totalPages}
        className="px-2 py-1 rounded border border-zinc-200 text-zinc-600 hover:bg-zinc-50 disabled:opacity-30 disabled:cursor-default cursor-pointer"
        onClick={() => onPageChange(page + 1)}
      >
        Next
      </button>
    </div>
  );
}

export function PaginatedFileList({
  pageFiles,
  page,
  totalPages,
  totalFiltered,
  pageSize,
  onPageChange,
}: {
  pageFiles: FileStatusEntry[];
  page: number;
  totalPages: number;
  totalFiltered: number;
  pageSize: number;
  onPageChange: (p: number) => void;
}) {
  const from = (page - 1) * pageSize + 1;
  const to = Math.min(page * pageSize, totalFiltered);

  return (
    <div>
      {/* Top controls */}
      <div className="flex items-center justify-between mb-2">
        {totalPages > 1 ? (
          <span className="text-[11px] text-zinc-400">
            Showing {from}&ndash;{to} of {totalFiltered}
          </span>
        ) : (
          <span />
        )}
        <PageControls page={page} totalPages={totalPages} onPageChange={onPageChange} />
      </div>

      {/* File table */}
      <FileTable files={pageFiles} />

      {/* Bottom controls */}
      {totalPages > 1 && (
        <div className="mt-2">
          <PageControls page={page} totalPages={totalPages} onPageChange={onPageChange} />
        </div>
      )}
    </div>
  );
}
