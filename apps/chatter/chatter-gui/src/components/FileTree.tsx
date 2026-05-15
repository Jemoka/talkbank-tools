/**
 * Collapsible file tree — left panel.
 *
 * Only shows files with errors (valid files are hidden). Sorted alphabetically.
 * Mirrors the TUI's file list behavior in
 * `crates/chatter-cli/src/ui/validation_tui/render.rs`.
 */

import { useMemo, useState, useCallback } from "react";
import type { FileEntry, TreeNode } from "../types";

interface Props {
  files: Map<string, FileEntry>;
  totalFiles: number;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
}

export default function FileTree({ files, totalFiles, selectedFile, onSelectFile }: Props) {
  // Filter to only files with errors (TUI parity: valid files are hidden)
  const filesWithErrors = useMemo(() => {
    const filtered = new Map<string, FileEntry>();
    for (const [key, entry] of files) {
      if (entry.diagnostics.length > 0) {
        filtered.set(key, entry);
      }
    }
    return filtered;
  }, [files]);

  const tree = useMemo(() => buildTree(filesWithErrors), [filesWithErrors]);
  const errorFileCount = filesWithErrors.size;

  if (totalFiles === 0) {
    return (
      <div className="file-tree-panel" style={{ padding: "16px", color: "var(--color-text-secondary)" }}>
        No files loaded
      </div>
    );
  }

  if (errorFileCount === 0) {
    return (
      <div className="file-tree-panel">
        <div className="tree-header">
          {totalFiles} files — all valid
        </div>
      </div>
    );
  }

  // If there's a single root directory, show it directly
  const roots = tree.children;

  return (
    <div className="file-tree-panel">
      <div className="tree-header">
        {errorFileCount} file{errorFileCount !== 1 ? "s" : ""} with errors / {totalFiles} total
      </div>
      {roots.map((node) => (
        <TreeNodeView
          key={node.path}
          node={node}
          selectedFile={selectedFile}
          onSelectFile={onSelectFile}
          depth={0}
        />
      ))}
    </div>
  );
}

function TreeNodeView({
  node,
  selectedFile,
  onSelectFile,
  depth,
}: {
  node: TreeNode;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  depth: number;
}) {
  const [expanded, setExpanded] = useState(true);
  const isDir = node.children.length > 0;
  const isSelected = node.file?.path === selectedFile;

  const errorCount = useMemo(() => countErrors(node), [node]);

  const handleClick = useCallback(() => {
    if (isDir) {
      setExpanded((prev) => !prev);
    } else if (node.file) {
      onSelectFile(node.file.path);
    }
  }, [isDir, node.file, onSelectFile]);

  return (
    <div className="tree-node">
      <div
        className={`tree-node-label ${isSelected ? "selected" : ""}`}
        style={{ paddingLeft: `${12 + depth * 16}px` }}
        onClick={handleClick}
      >
        {isDir && (
          <span className="tree-toggle">{expanded ? "\u25BE" : "\u25B8"}</span>
        )}
        {!isDir && (
          <span style={{ color: isSelected ? "#fff" : "var(--color-error)" }}>
            {"\u2717"}
          </span>
        )}
        <span>{node.name}</span>
        {isDir && errorCount > 0 && (
          <span className="badge error">{errorCount}</span>
        )}
        {!isDir && node.file && node.file.diagnostics.length > 0 && (
          <span className="badge error">{node.file.diagnostics.length}</span>
        )}
      </div>
      {isDir && expanded && (
        <div className="tree-children">
          {node.children.map((child) => (
            <TreeNodeView
              key={child.path}
              node={child}
              selectedFile={selectedFile}
              onSelectFile={onSelectFile}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function countErrors(node: TreeNode): number {
  if (node.file) return node.file.diagnostics.length;
  return node.children.reduce((sum, child) => sum + countErrors(child), 0);
}

/** Build a directory tree from flat file entries. Sorted alphabetically. */
function buildTree(files: Map<string, FileEntry>): TreeNode {
  const root: TreeNode = { name: "", path: "", children: [], file: null };

  // Sort alphabetically by path (TUI parity)
  const sortedEntries = [...files.values()].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  for (const file of sortedEntries) {
    const parts = file.name.split("/");
    let current = root;

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const isLast = i === parts.length - 1;

      if (isLast) {
        current.children.push({
          name: part,
          path: file.path,
          children: [],
          file,
        });
      } else {
        let child = current.children.find(
          (c) => c.name === part && c.children.length >= 0 && !c.file,
        );
        if (!child) {
          child = {
            name: part,
            path: parts.slice(0, i + 1).join("/"),
            children: [],
            file: null,
          };
          current.children.push(child);
        }
        current = child;
      }
    }
  }

  // Prune empty directories (can happen after filtering valid files)
  pruneEmptyDirs(root);

  return root;
}

/** Remove directory nodes that have no children after filtering */
function pruneEmptyDirs(node: TreeNode): void {
  node.children = node.children.filter((child) => {
    if (child.file) return true; // Keep files
    pruneEmptyDirs(child);
    return child.children.length > 0; // Keep dirs only if they have children
  });
}
