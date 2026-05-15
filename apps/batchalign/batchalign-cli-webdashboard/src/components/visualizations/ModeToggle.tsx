/** Toggle between Static (educational) and Live (job trace) modes. */
export function ModeToggle({ mode }: { mode: "static" | "live" }) {
  return (
    <div className="flex items-center gap-2 text-xs">
      <span
        className={`px-2 py-0.5 rounded ${
          mode === "static"
            ? "bg-violet-100 text-violet-700"
            : "bg-gray-100 text-gray-500"
        }`}
      >
        Static
      </span>
      <span
        className={`px-2 py-0.5 rounded ${
          mode === "live"
            ? "bg-blue-100 text-blue-700"
            : "bg-gray-100 text-gray-500"
        }`}
      >
        Live
      </span>
    </div>
  );
}
