// HTTP client for the local daemon sidecar.
//
// Until `just batchalign gen-openapi` produces `protocol/openapi.gen.ts`,
// this client uses hand-rolled fetch wrappers. After codegen lands, the
// `client` export will be replaced with an `openapi-fetch` instance
// parameterized on `paths` from the generated module:
//
//   import createClient from "openapi-fetch";
//   import type { paths } from "./protocol/openapi.gen";
//   export const client = createClient<paths>({ baseUrl });
//
// The shape below is what the daemon's FastAPI app exposes today
// (python/batchalign/api.py); see the plan §4.5 for the full contract.

import type { CapabilitiesJson } from "./store";

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
  const url = `${getBaseUrl()}${path}`;
  const init: RequestInit = { method };
  if (body !== undefined) {
    init.headers = { "content-type": "application/json" };
    init.body = JSON.stringify(body);
  }
  const resp = await fetch(url, init);
  if (!resp.ok) {
    const text = await resp.text().catch(() => "");
    throw new Error(`${method} ${path} → ${resp.status}: ${text}`);
  }
  return (await resp.json()) as T;
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
