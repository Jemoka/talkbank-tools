import { createContext, useContext, type ReactNode } from "react";
import {
  desktopRuntime,
  type ClanCapability,
  type DesktopEnvironmentCapability,
  type DesktopRuntime,
  type ExportCapability,
  type ValidationRunnerCapability,
  type ValidationTargetCapability,
} from "./desktopRuntime";

const DesktopRuntimeContext = createContext<DesktopRuntime>(desktopRuntime);

interface DesktopRuntimeProviderProps {
  children: ReactNode;
  runtime?: DesktopRuntime;
}

export function DesktopRuntimeProvider({
  children,
  runtime = desktopRuntime,
}: DesktopRuntimeProviderProps) {
  return (
    <DesktopRuntimeContext.Provider value={runtime}>
      {children}
    </DesktopRuntimeContext.Provider>
  );
}

function useDesktopRuntimeContext(): DesktopRuntime {
  return useContext(DesktopRuntimeContext);
}

export function useDesktopEnvironmentCapability(): DesktopEnvironmentCapability {
  return useDesktopRuntimeContext().environment;
}

export function useValidationRunnerCapability(): ValidationRunnerCapability {
  return useDesktopRuntimeContext().validationRunner;
}

export function useValidationTargetCapability(): ValidationTargetCapability {
  return useDesktopRuntimeContext().validationTarget;
}

export function useClanCapability(): ClanCapability {
  return useDesktopRuntimeContext().clan;
}

export function useExportCapability(): ExportCapability {
  return useDesktopRuntimeContext().exports;
}
