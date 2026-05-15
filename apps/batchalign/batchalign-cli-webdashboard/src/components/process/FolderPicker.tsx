/** Native folder picker with drag-and-drop zone.
 *
 * In desktop mode, uses the Tauri dialog plugin for a native folder picker and
 * the desktop file capability for recursive discovery. Falls back to a disabled
 * state in web mode with a message directing users to the desktop app.
 */

import { useState, type DragEvent } from "react";
import {
  useDesktopEnvironment,
  useDesktopFiles,
} from "../../desktop/DesktopContext";

interface FolderPickerProps {
  /** Label shown above the picker. */
  label: string;
  /** File extensions to filter for (e.g. ["cha", "wav"]). Empty = all files. */
  extensions: string[];
  /** Called with the selected folder path and discovered file paths. */
  onSelect: (folder: string, files: string[]) => void;
  /** Currently selected folder path. */
  selectedFolder: string | null;
  /** Number of discovered files in the selected folder. */
  fileCount: number;
  /** Optional dialog title. */
  dialogTitle?: string;
}

export function FolderPicker({
  label,
  extensions,
  onSelect,
  selectedFolder,
  fileCount,
  dialogTitle,
}: FolderPickerProps) {
  const environment = useDesktopEnvironment();
  const files = useDesktopFiles();
  const [isLoading, setIsLoading] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    if (!environment.isDesktop) return;

    // Tauri webview provides paths via dataTransfer
    const items = e.dataTransfer?.files;
    if (!items || items.length === 0) return;

    // Use the first dropped item's path (webkitRelativePath or name)
    // In Tauri, the file path comes through the File API
    const firstFile = items[0];
    const path = (firstFile as File & { path?: string }).path;
    if (!path) return;

    setIsLoading(true);
    try {
      const discovered = await files.discoverFiles(path, extensions);
      onSelect(path, discovered);
    } catch (err) {
      console.error("Drop discovery failed:", err);
    } finally {
      setIsLoading(false);
    }
  }

  async function handleClick() {
    if (!environment.isDesktop) return;
    setIsLoading(true);
    try {
      const folder = await files.pickFolder(dialogTitle ?? label);
      if (!folder) return;
      const discovered = await files.discoverFiles(folder, extensions);
      onSelect(folder, discovered);
    } catch (err) {
      console.error("Folder pick failed:", err);
    } finally {
      setIsLoading(false);
    }
  }

  /** Extract the last path component for compact display. */
  function folderName(path: string): string {
    const parts = path.replace(/\/+$/, "").split(/[/\\]/);
    return parts[parts.length - 1] || path;
  }

  return (
    <div>
      <label className="block text-sm font-medium text-gray-700 mb-1.5">
        {label}
      </label>

      <button
        type="button"
        onClick={handleClick}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        disabled={!environment.isDesktop || isLoading}
        className={`w-full border-2 border-dashed rounded-lg p-6 text-center
          transition-colors cursor-pointer
          disabled:opacity-50 disabled:cursor-not-allowed ${
            isDragOver
              ? "border-indigo-500 bg-indigo-50"
              : "border-gray-300 hover:border-indigo-400 hover:bg-indigo-50/50"
          }`}
      >
        {isLoading ? (
          <span className="text-sm text-gray-500">Scanning files...</span>
        ) : selectedFolder ? (
          <div>
            <div className="text-sm font-medium text-gray-800">
              {folderName(selectedFolder)}
            </div>
            <div className="text-xs text-gray-500 mt-1">
              {fileCount} file{fileCount !== 1 ? "s" : ""} found
            </div>
            <div className="text-xs text-gray-400 mt-0.5 truncate max-w-md mx-auto">
              {selectedFolder}
            </div>
          </div>
        ) : (
          <div>
            <div className="text-sm text-gray-600">
              {environment.isDesktop
                ? "Click to choose a folder, or drag one here"
                : "Folder selection requires the desktop app"}
            </div>
            {extensions.length > 0 && (
              <div className="text-xs text-gray-400 mt-1">
                Looking for: {extensions.map((e) => `.${e}`).join(", ")}
              </div>
            )}
          </div>
        )}
      </button>
    </div>
  );
}
