/** First-time onboarding hint shown to users who completed setup but
 * haven't processed any files yet.
 *
 * Shows a brief 3-step guide overlaid on the home screen. Dismisses
 * permanently on click or after the user submits their first job.
 * Uses localStorage to track dismissal so it only shows once.
 */

import { useState, useEffect } from "react";

const STORAGE_KEY = "batchalign-onboarding-dismissed";

interface OnboardingOverlayProps {
  /** True when there are any jobs in the store (user has processed before). */
  hasJobs: boolean;
}

export function OnboardingOverlay({ hasJobs }: OnboardingOverlayProps) {
  const [dismissed, setDismissed] = useState(true); // Start hidden to avoid flash

  useEffect(() => {
    // Don't show if user has already processed jobs
    if (hasJobs) return;
    // Check localStorage for permanent dismissal
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) {
      setDismissed(false);
    }
  }, [hasJobs]);

  if (dismissed || hasJobs) return null;

  function handleDismiss() {
    localStorage.setItem(STORAGE_KEY, "true");
    setDismissed(true);
  }

  return (
    <div className="bg-indigo-50 border border-indigo-200 rounded-lg p-5 relative">
      <button
        onClick={handleDismiss}
        className="absolute top-3 right-3 text-indigo-400 hover:text-indigo-600 transition-colors"
        aria-label="Dismiss"
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>

      <h3 className="text-sm font-semibold text-indigo-800 mb-3">
        How it works
      </h3>

      <div className="flex gap-4">
        <Step number={1} title="Pick a task" description="Choose what you want to do with your files" />
        <Step number={2} title="Select files" description="Pick a folder with your input files" />
        <Step number={3} title="Watch progress" description="See results as each file is processed" />
      </div>
    </div>
  );
}

function Step({ number, title, description }: { number: number; title: string; description: string }) {
  return (
    <div className="flex-1 text-center">
      <div className="inline-flex items-center justify-center w-8 h-8 rounded-full bg-indigo-600 text-white text-sm font-semibold mb-2">
        {number}
      </div>
      <div className="text-sm font-medium text-indigo-800">{title}</div>
      <div className="text-xs text-indigo-600 mt-0.5">{description}</div>
    </div>
  );
}
