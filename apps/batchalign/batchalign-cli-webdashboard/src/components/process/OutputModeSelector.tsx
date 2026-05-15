/** Toggle between "save to separate folder" and "modify in place" output modes.
 *
 * Defaults to separate folder (safer). When "modify in place" is selected,
 * shows a warning that original files will be overwritten.
 */

export type OutputMode = "separate" | "in-place";

interface OutputModeSelectorProps {
  mode: OutputMode;
  onChange: (mode: OutputMode) => void;
}

export function OutputModeSelector({ mode, onChange }: OutputModeSelectorProps) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-700 mb-1.5">
        Output location
      </label>

      <div className="flex gap-1 bg-gray-100 rounded-lg p-1">
        <button
          type="button"
          onClick={() => onChange("separate")}
          className={`flex-1 text-sm py-1.5 px-3 rounded-md transition-colors ${
            mode === "separate"
              ? "bg-white text-gray-800 shadow-sm font-medium"
              : "text-gray-500 hover:text-gray-700"
          }`}
        >
          Save to separate folder
        </button>
        <button
          type="button"
          onClick={() => onChange("in-place")}
          className={`flex-1 text-sm py-1.5 px-3 rounded-md transition-colors ${
            mode === "in-place"
              ? "bg-white text-gray-800 shadow-sm font-medium"
              : "text-gray-500 hover:text-gray-700"
          }`}
        >
          Modify in place
        </button>
      </div>

      {mode === "in-place" && (
        <p className="text-xs text-amber-600 mt-2">
          Original files will be overwritten. Make sure you have backups.
        </p>
      )}
    </div>
  );
}
