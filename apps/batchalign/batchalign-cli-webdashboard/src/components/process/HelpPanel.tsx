/** Slide-out help panel with command descriptions and FAQ.
 *
 * Triggered by a "?" button in the header. Slides in from the right edge
 * as an overlay. Contains brief descriptions of each command and answers
 * to common questions researchers have.
 */

import { useEffect, useRef } from "react";
import { COMMANDS, type CommandDef } from "./CommandPicker";
import { commandStyle } from "../../utils";

interface HelpPanelProps {
  open: boolean;
  onClose: () => void;
}

const FAQ: Array<{ q: string; a: string }> = [
  {
    q: "What is CHAT format?",
    a: "CHAT is the standard transcript format used by TalkBank and CLAN. Each line starts with a speaker code (like *CHI: or *MOT:) followed by what they said. Additional lines starting with % contain analysis like grammar tags or timing.",
  },
  {
    q: "What file formats are supported?",
    a: "Most commands work with CHAT files (.cha). For transcription, audio and video files are accepted (.wav, .mp3, .mp4, .m4a, .flac, .ogg, .webm).",
  },
  {
    q: "Where do output files go?",
    a: "By default, output goes to a separate folder you choose. You can also select 'Modify in place' to overwrite the originals (make backups first!).",
  },
  {
    q: "When should I use 'Add Grammar'?",
    a: "Use this when you need part-of-speech tags (%mor tier) and grammatical relations (%gra tier) — required by CLAN analysis commands like MLU, DSS, FREQ, and COMBO. Run it after you have a transcript.",
  },
  {
    q: "When should I use 'Align to Audio'?",
    a: "Use this when you have a transcript and matching audio but no word-level timing. After alignment, you can click any word in CLAN to hear exactly when it was said. Also needed for accurate utterance-level playback.",
  },
  {
    q: "What's the difference between Rev.AI and Whisper?",
    a: "Rev.AI is a cloud service — fast and accurate but requires an API key and costs money per minute of audio. Whisper runs locally on your machine — free but slower, especially without a GPU. You can change your default in Settings (gear icon).",
  },
  {
    q: "How long does processing take?",
    a: "It depends on the command and file size. Grammar tagging is fast (seconds per file). Transcription is slower — Rev.AI processes roughly in real-time, Whisper can take 2-5x the audio duration. The first run downloads ML models (~2 GB), which is a one-time cost.",
  },
  {
    q: "Can I process files in different languages?",
    a: "Yes, but each job processes one language at a time. Choose the language when you start a job. Batchalign supports many languages including English, Spanish, French, German, Chinese, Japanese, Korean, and Cantonese.",
  },
  {
    q: "Can I process files while others are running?",
    a: "Yes, you can submit multiple jobs. Each one processes independently and you can track them all in the Dashboard view.",
  },
  {
    q: "What if processing fails on some files?",
    a: "Error messages explain what went wrong and suggest fixes. Common causes: invalid CHAT format, missing audio files, or low memory. Successfully processed files in the same batch are still saved. You can fix the problem files and re-process just those.",
  },
  {
    q: "How do I cancel a running job?",
    a: "Click the 'Cancel' link in the top-right corner of the progress screen. Files that already finished processing are kept.",
  },
  {
    q: "How do I change my ASR engine or API key?",
    a: "Click the gear icon in the top bar to open Settings. You can switch between Rev.AI and Whisper and update your API key.",
  },
];

function CommandDescription({ cmd }: { cmd: CommandDef }) {
  const [bg, text] = commandStyle(cmd.id);
  return (
    <div className="flex items-start gap-3">
      <span className={`text-xs font-medium px-1.5 py-0.5 rounded flex-shrink-0 mt-0.5 ${bg} ${text}`}>
        {cmd.id}
      </span>
      <div>
        <div className="text-sm font-medium text-gray-800">{cmd.label}</div>
        <div className="text-xs text-gray-500">{cmd.description}</div>
      </div>
    </div>
  );
}

export function HelpPanel({ open, onClose }: HelpPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on Escape and trap focus inside panel
  useEffect(() => {
    if (!open) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      // Focus trap: Tab cycles within the panel
      if (e.key === "Tab" && panelRef.current) {
        const focusable = panelRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    // Auto-focus the close button on open
    const closeBtn = panelRef.current?.querySelector<HTMLElement>("button");
    closeBtn?.focus();
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/20 z-40"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Panel */}
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="Help"
        className="fixed right-0 top-0 h-full w-96 max-w-[90vw] bg-white shadow-xl z-50 overflow-y-auto"
      >
        <div className="p-6 space-y-6">
          {/* Header */}
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold text-gray-900">Help</h2>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-gray-600 transition-colors"
              aria-label="Close help"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* Commands */}
          <div>
            <h3 className="text-sm font-semibold text-gray-700 mb-3">
              Available Commands
            </h3>
            <div className="space-y-3">
              {COMMANDS.map((cmd) => (
                <CommandDescription key={cmd.id} cmd={cmd} />
              ))}
            </div>
          </div>

          {/* FAQ */}
          <div>
            <h3 className="text-sm font-semibold text-gray-700 mb-3">
              Frequently Asked Questions
            </h3>
            <div className="space-y-4">
              {FAQ.map(({ q, a }) => (
                <div key={q}>
                  <div className="text-sm font-medium text-gray-800">{q}</div>
                  <div className="text-xs text-gray-500 mt-1">{a}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
