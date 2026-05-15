/** Re-export server types from auto-generated OpenAPI definitions. */

import type { components } from "./generated/api";

// Server types (generated from the repo-root openapi.json via OpenAPI)
export type JobStatusValue = components["schemas"]["JobStatus"];
export type FileStatusEntry = components["schemas"]["FileStatusEntry"];
export type HealthResponse = components["schemas"]["HealthResponse"];
export type JobControlPlaneInfo = components["schemas"]["JobControlPlaneInfo"];

// JobInfo: override file_statuses to required (server always sends it),
// add client-side `server` enrichment.
export type JobInfo = Omit<components["schemas"]["JobInfo"], "file_statuses"> & {
  file_statuses: FileStatusEntry[];
  server?: string;
};

// Enriched with client-side `server` field (not in OpenAPI)
export type JobListItem = components["schemas"]["JobListItem"] & {
  server?: string;
};

// Client-only: WebSocket message union (not in OpenAPI)
export type WSMessage =
  | { type: "snapshot"; jobs: JobListItem[]; health: HealthResponse }
  | { type: "job_update"; job: JobListItem }
  | {
      type: "file_update";
      job_id: string;
      file: FileStatusEntry;
      completed_files: number;
    }
  | { type: "job_deleted"; job_id: string };
