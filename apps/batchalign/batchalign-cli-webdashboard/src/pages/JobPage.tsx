/** Route shell for `/dashboard/jobs/:id`.
 *
 * The page is intentionally thin: it parses routing state, delegates all data
 * loading to `useJobPageController`, and hands the resolved view model to the
 * presentational detail component.
 */
import { useLocation, useRoute } from "wouter";
import { Layout } from "../components/Layout";
import { JobDetailPageView } from "../components/JobDetailPageView";
import { useJobPageController } from "../hooks/useJobPageController";

/** Render the job-detail route or the small route-level loading/error states. */
export function JobPage() {
  const [, params] = useRoute("/dashboard/jobs/:id");
  const [, navigate] = useLocation();
  const jobId = params?.id ?? "";
  const { loading, detail, wsJob, multiServer, effectiveServer, serverBase } =
    useJobPageController(jobId);

  if (!jobId) {
    return (
      <Layout>
        <p className="text-zinc-500 text-sm">Job not found.</p>
      </Layout>
    );
  }

  if (loading) {
    return (
      <Layout>
        <p className="text-zinc-400 text-sm">Loading...</p>
      </Layout>
    );
  }

  if (!detail) {
    return (
      <Layout>
        <p className="text-zinc-500 text-sm">Job not found.</p>
      </Layout>
    );
  }

  return (
    <JobDetailPageView
      detail={detail}
      wsJob={wsJob}
      multiServer={multiServer}
      effectiveServer={effectiveServer}
      serverBase={serverBase}
      onDeleted={() => navigate("/dashboard")}
    />
  );
}
