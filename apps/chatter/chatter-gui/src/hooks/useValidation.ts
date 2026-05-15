import { useCallback, useEffect, useRef, useState } from "react";
import { useValidationRunnerCapability } from "../runtime/DesktopRuntimeContext";
import type { ValidationRun } from "../runtime/desktopRuntime";
import {
  applyValidationEvent,
  createInitialValidationState,
  relativeDisplayName,
  type ValidationState,
} from "./validationState";

export type { Phase, ValidationState } from "./validationState";

/**
 * Hook that manages all validation state from the Tauri event stream.
 *
 * Accumulates per-file results from the desktop runtime's validation event stream.
 */
export function useValidation() {
  const validationRunner = useValidationRunnerCapability();
  const [state, setState] = useState<ValidationState>(createInitialValidationState);
  const runRef = useRef<ValidationRun | null>(null);
  /** Selected validation target used for computing relative display names */
  const rootRef = useRef<string>("");

  const disposeRun = useCallback(() => {
    runRef.current?.dispose();
    runRef.current = null;
  }, []);

  const relativeName = useCallback((fullPath: string): string => {
    return relativeDisplayName(fullPath, rootRef.current);
  }, []);

  const startValidation = useCallback(async (path: string) => {
    disposeRun();

    rootRef.current = path;

    setState({ ...createInitialValidationState(), phase: "discovering" });

    try {
      runRef.current = await validationRunner.startValidation(path, (event) => {
        setState((prev) => applyValidationEvent(prev, event, relativeName));
      });
    } catch (err) {
      console.error("validate command failed:", err);
      window.alert(`Validation failed: ${String(err)}`);
      disposeRun();
      setState((prev) => ({ ...prev, phase: "finished" }));
    }
  }, [disposeRun, relativeName, validationRunner]);

  const cancelValidation = useCallback(async () => {
    try {
      await runRef.current?.cancel();
    } catch (err) {
      console.error("cancel failed:", err);
    }
  }, []);

  const reset = useCallback(() => {
    rootRef.current = "";
    disposeRun();
    setState(createInitialValidationState());
  }, [disposeRun]);

  useEffect(() => () => {
    disposeRun();
  }, [disposeRun]);

  return { state, startValidation, cancelValidation, reset };
}
