import type { FileCounts, FilterTab } from "../hooks/useFileFilters";

const TABS: Array<{ key: FilterTab; label: string }> = [
  { key: "all", label: "All" },
  { key: "error", label: "Errors" },
  { key: "processing", label: "Processing" },
  { key: "done", label: "Done" },
  { key: "queued", label: "Queued" },
];

export function FilterTabs({
  activeTab,
  counts,
  searchQuery,
  onTabChange,
  onSearchChange,
}: {
  activeTab: FilterTab;
  counts: FileCounts;
  searchQuery: string;
  onTabChange: (tab: FilterTab) => void;
  onSearchChange: (q: string) => void;
}) {
  return (
    <div className="flex items-center gap-2 flex-wrap">
      {/* Tabs */}
      <div className="flex items-center gap-0.5 bg-zinc-100 rounded-lg p-0.5" role="tablist" aria-label="Filter files by status">
        {TABS.map(({ key, label }) => {
          const count = counts[key];
          const isActive = activeTab === key;
          const isEmpty = count === 0 && key !== "all";
          return (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={isActive}
              disabled={isEmpty}
              className={`px-2.5 py-1 rounded-md text-xs font-medium transition-colors cursor-pointer ${
                isActive
                  ? "bg-white text-zinc-800 shadow-sm"
                  : isEmpty
                    ? "text-zinc-300 cursor-default"
                    : "text-zinc-500 hover:text-zinc-700"
              }`}
              onClick={() => onTabChange(key)}
            >
              {label}
              <span className={`ml-1 font-mono ${isActive ? "text-zinc-600" : "text-zinc-400"}`}>
                {count}
              </span>
            </button>
          );
        })}
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Search */}
      <label className="sr-only" htmlFor="file-search">Search files</label>
      <input
        id="file-search"
        type="text"
        placeholder="Search files..."
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        className="w-48 px-2.5 py-1.5 text-xs border border-zinc-200 rounded-md bg-white placeholder:text-zinc-400 focus:outline-none focus:ring-1 focus:ring-zinc-300"
      />
    </div>
  );
}
