/** Main form composing command selection, file picking, output mode, and submission.
 *
 * This is the primary user flow for the desktop app. A researcher picks a
 * command, selects an input folder, optionally picks an output folder, and
 * starts processing. Desktop-only file interactions stay behind the focused
 * file capability so the form never imports raw Tauri APIs.
 */

import { useState } from "react";
import { CommandPicker, type CommandDef } from "./CommandPicker";
import { FolderPicker } from "./FolderPicker";
import { OutputModeSelector, type OutputMode } from "./OutputModeSelector";
import { ProcessingProgress } from "./ProcessingProgress";
import { useSubmitJob } from "../../hooks/useSubmitJob";
import {
  useDesktopEnvironment,
  useDesktopFiles,
} from "../../desktop/DesktopContext";

/** File extensions relevant to each command. */
function extensionsForCommand(command: string): string[] {
  switch (command) {
    case "transcribe":
      return ["wav", "mp3", "mp4", "m4a", "flac", "ogg", "webm"];
    default:
      return ["cha"];
  }
}

type FormStep = "pick-command" | "configure" | "processing";

interface ProcessFormProps {
  /** Whether the server is ready to accept jobs (from lifecycle hook). */
  isServerReady: boolean;
}

export function ProcessForm({ isServerReady }: ProcessFormProps) {
  const environment = useDesktopEnvironment();
  const files = useDesktopFiles();
  const [step, setStep] = useState<FormStep>("pick-command");
  const [command, setCommand] = useState<CommandDef | null>(null);
  const [inputFolder, setInputFolder] = useState<string | null>(null);
  const [inputFiles, setInputFiles] = useState<string[]>([]);
  const [outputMode, setOutputMode] = useState<OutputMode>("separate");
  const [outputFolder, setOutputFolder] = useState<string | null>(null);
  const [lang, setLang] = useState("eng");
  const [activeJobId, setActiveJobId] = useState<string | null>(null);

  const submitJob = useSubmitJob();

  function handleCommandSelect(cmd: CommandDef) {
    setCommand(cmd);
    setStep("configure");
    // Reset file selections when switching commands
    setInputFolder(null);
    setInputFiles([]);
    setOutputFolder(null);
  }

  function handleInputSelect(folder: string, discovered: string[]) {
    setInputFolder(folder);
    setInputFiles(discovered);
  }

  async function handleOutputSelect() {
    const folder = await files.pickOutputFolder();
    if (folder) setOutputFolder(folder);
  }

  /** Build output paths: either mirror structure into output folder, or overwrite in place.
   *
   * For the separate-folder case, subdirectory structure relative to the
   * input folder is preserved. For example, if inputFolder is
   * `.../French` and a file is `.../French/Newcastle/S1.wav`, the output
   * path is `<dest>/Newcastle/S1.cha`. This prevents name collisions for
   * corpora where different subdirectories contain identically-named files.
   */
  function buildOutputPaths(): string[] {
    if (outputMode === "in-place") {
      return inputFiles;
    }
    const dest = outputFolder ?? inputFolder;
    if (!dest) return inputFiles;

    // Strip trailing slashes from the input folder so the prefix is clean.
    const folderPrefix = (inputFolder ?? "").replace(/\/+$/, "") + "/";

    return inputFiles.map((inputPath) => {
      // Compute the path relative to the input folder, falling back to just
      // the filename if the path doesn't start with the expected prefix.
      const relativePath = inputPath.startsWith(folderPrefix)
        ? inputPath.slice(folderPrefix.length)
        : (inputPath.split(/[/\\]/).pop() ?? inputPath);

      // For transcribe, the input is audio but the output is a .cha transcript.
      const outputRelPath =
        command?.id === "transcribe"
          ? relativePath.replace(/\.[^.]+$/, ".cha")
          : relativePath;

      return `${dest}/${outputRelPath}`;
    });
  }

  async function handleSubmit() {
    if (!command || inputFiles.length === 0) return;

    const outputPaths = buildOutputPaths();

    try {
      const job = await submitJob.mutateAsync({
        command: command.id,
        lang: command.needsLang ? lang : undefined,
        sourcePaths: inputFiles,
        outputPaths,
        sourceDir: inputFolder ?? undefined,
      });
      setActiveJobId(job.job_id);
      setStep("processing");
    } catch {
      // Error state is exposed via submitJob.isError / submitJob.error
    }
  }

  function handleReset() {
    setStep("pick-command");
    setCommand(null);
    setInputFolder(null);
    setInputFiles([]);
    setOutputFolder(null);
    setActiveJobId(null);
    submitJob.reset();
  }

  // Processing screen
  if (step === "processing" && activeJobId && command) {
    return (
      <ProcessingProgress
        jobId={activeJobId}
        totalFiles={inputFiles.length}
        command={command.id}
        outputFolder={outputMode === "separate" ? outputFolder : inputFolder}
        onReset={handleReset}
      />
    );
  }

  // Configure screen
  if (step === "configure" && command) {
    const canSubmit =
      isServerReady && inputFiles.length > 0 && !submitJob.isPending;
    const needsOutputFolder = outputMode === "separate";

    return (
      <div className="max-w-xl mx-auto space-y-5">
        {/* Header */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => setStep("pick-command")}
            className="text-gray-400 hover:text-gray-600 transition-colors"
            aria-label="Back"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <h2 className="text-lg font-semibold text-gray-800">
            {command.label}
          </h2>
        </div>

        {/* Input folder */}
        <FolderPicker
          label="Choose input files"
          extensions={extensionsForCommand(command.id)}
          onSelect={handleInputSelect}
          selectedFolder={inputFolder}
          fileCount={inputFiles.length}
          dialogTitle={`Select ${command.label} input folder`}
        />

        {/* Output mode */}
        <OutputModeSelector mode={outputMode} onChange={setOutputMode} />

        {/* Output folder picker (when separate mode) */}
        {needsOutputFolder && (
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1.5">
              Output folder
            </label>
            {outputFolder ? (
              <button
                type="button"
                onClick={handleOutputSelect}
                className="w-full text-left border border-gray-200 rounded-lg p-3 hover:border-gray-300 transition-colors"
              >
                <div className="text-sm text-gray-700 truncate">{outputFolder}</div>
                <div className="text-xs text-gray-400 mt-0.5">Click to change</div>
              </button>
            ) : (
              <button
                type="button"
                onClick={handleOutputSelect}
                disabled={!environment.isDesktop}
                className="w-full border-2 border-dashed border-gray-300 rounded-lg p-4 text-center
                  hover:border-indigo-400 hover:bg-indigo-50/50 transition-colors
                  disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <span className="text-sm text-gray-600">
                  Click to choose output folder
                </span>
              </button>
            )}
          </div>
        )}

        {/* Language selector */}
        {command.needsLang && (
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1.5">
              Language
            </label>
            <select
              value={lang}
              onChange={(e) => setLang(e.target.value)}
              className="block w-full border border-gray-300 rounded-lg px-3 py-2 text-sm
                focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
            >
              {/* Germanic */}
              <option value="eng">English</option>
              <option value="deu">German</option>
              <option value="nld">Dutch</option>
              <option value="afr">Afrikaans</option>
              <option value="swe">Swedish</option>
              <option value="dan">Danish</option>
              <option value="nor">Norwegian</option>
              {/* Romance */}
              <option value="spa">Spanish</option>
              <option value="fra">French</option>
              <option value="ita">Italian</option>
              <option value="por">Portuguese</option>
              <option value="cat">Catalan</option>
              {/* Slavic */}
              <option value="rus">Russian</option>
              <option value="pol">Polish</option>
              <option value="ces">Czech</option>
              <option value="slk">Slovak</option>
              <option value="ukr">Ukrainian</option>
              <option value="bul">Bulgarian</option>
              {/* East Asian */}
              <option value="zho">Chinese (Mandarin)</option>
              <option value="yue">Cantonese</option>
              <option value="jpn">Japanese</option>
              <option value="kor">Korean</option>
              {/* Other */}
              <option value="ara">Arabic</option>
              <option value="heb">Hebrew</option>
              <option value="tur">Turkish</option>
              <option value="fin">Finnish</option>
              <option value="est">Estonian</option>
              <option value="eus">Basque</option>
              <option value="ind">Indonesian</option>
              <option value="vie">Vietnamese</option>
              <option value="tha">Thai</option>
              <option value="hin">Hindi</option>
              <option value="ben">Bengali</option>
              <option value="mar">Marathi</option>
              <option value="tel">Telugu</option>
              <option value="tam">Tamil</option>
              <option value="mal">Malayalam</option>
            </select>
          </div>
        )}

        {/* Error display */}
        {submitJob.isError && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">
            {submitJob.error?.message ?? "Failed to submit job"}
          </div>
        )}

        {/* Submit button */}
        <button
          type="button"
          onClick={handleSubmit}
          disabled={!canSubmit}
          className="w-full py-3 text-sm font-semibold text-white bg-indigo-600 rounded-lg
            hover:bg-indigo-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {submitJob.isPending
            ? "Submitting..."
            : `Start Processing ${inputFiles.length > 0 ? `(${inputFiles.length} files)` : ""}`}
        </button>

        {!isServerReady && (
          <p className="text-xs text-amber-600 text-center">
            Waiting for server to start...
          </p>
        )}
      </div>
    );
  }

  // Home screen (pick command)
  return <CommandPicker onSelect={handleCommandSelect} />;
}
