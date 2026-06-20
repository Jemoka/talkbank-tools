// HTTP client for the local daemon sidecar.
//
// Routes every request through the Rust-side `daemon_request` command
// rather than the webview's native `fetch`. macOS WebKit (the Tauri
// webview) blocks fetches from the `tauri://localhost` origin to
// `http://127.0.0.1:<port>` with a generic `TypeError: Load failed`;
// the Rust relay sidesteps the WebKit network stack entirely.
//
import { invoke } from "@tauri-apps/api/core";
import type { CapabilitiesJson } from "./store";

// Kept for legacy callers; the Rust relay learns the port from
// AppState, so the JS side no longer needs it. Still exported so
// existing imports (bridge.ts) compile.
let baseUrl: string | null = null;

export function setBaseUrl(url: string) {
  baseUrl = url.replace(/\/$/, "");
}

export function getBaseUrl(): string {
  if (!baseUrl) throw new Error("daemon base URL not set yet");
  return baseUrl;
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  return (await invoke("daemon_request", {
    method,
    path,
    body: body ?? null,
  })) as T;
}

export interface HealthJson {
  ok: boolean;
  recipes: string[];
  backend_kinds: string[];
}

export async function fetchHealth(): Promise<HealthJson> {
  return request<HealthJson>("GET", "/health");
}

export async function fetchCapabilities(): Promise<CapabilitiesJson> {
  return request<CapabilitiesJson>("GET", "/capabilities");
}

/** Submit a job for `recipe`; returns the daemon's job id. */
export async function submitRecipe(
  recipe: string,
  body: Record<string, unknown>,
): Promise<{ job_id: string }> {
  return request<{ job_id: string }>("POST", `/recipes/${recipe}`, body);
}

export interface JobStatusJson {
  id: string;
  recipe: string;
  state: "pending" | "running" | "completed" | "failed" | "cancelled";
  progress: number;
  created_at: number;
  started_at: number | null;
  finished_at: number | null;
  error: string | null;
}

export async function fetchJobStatus(jobId: string): Promise<JobStatusJson> {
  return request<JobStatusJson>("GET", `/jobs/${jobId}`);
}

export async function cancelJob(jobId: string): Promise<void> {
  await request<{ cancelled: boolean }>("DELETE", `/jobs/${jobId}`);
}
