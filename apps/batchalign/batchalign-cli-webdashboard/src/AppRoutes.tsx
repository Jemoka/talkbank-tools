/** Static route table for the dashboard SPA.
 *
 * The `/process` route is the end-user processing flow (default in desktop mode).
 * The `/dashboard` routes are the fleet monitoring surface for power users.
 *
 * The route layer is intentionally declarative: pages are responsible for
 * their own controller logic, while this module only answers "which page owns
 * which URL?". That separation keeps routing changes from being tangled with
 * state synchronization or view rendering code.
 */

import { Router, Route, Switch, Redirect } from "wouter";
import { Layout } from "./components/Layout";
import { ProcessPage } from "./pages/ProcessPage";
import { DashboardPage } from "./pages/DashboardPage";
import { JobPage } from "./pages/JobPage";
import { VisualizationsIndex } from "./pages/visualizations/VisualizationsIndex";
import { RetokenizationPage } from "./pages/visualizations/RetokenizationPage";
import { DPAlignmentPage } from "./pages/visualizations/DPAlignmentPage";
import { ASRPipelinePage } from "./pages/visualizations/ASRPipelinePage";
import { FATimelinePage } from "./pages/visualizations/FATimelinePage";
import { useDesktopEnvironment } from "./desktop/DesktopContext";

export function AppRoutes() {
  const environment = useDesktopEnvironment();

  return (
    <Router>
      <Switch>
        {/* Process flow — end-user facing */}
        <Route path="/process">
          <Layout>
            <ProcessPage />
          </Layout>
        </Route>

        {/* Dashboard — fleet monitoring for power users */}
        <Route path="/dashboard/visualizations/retokenize">
          <RetokenizationPage />
        </Route>
        <Route path="/dashboard/visualizations/dp-alignment">
          <DPAlignmentPage />
        </Route>
        <Route path="/dashboard/visualizations/asr-pipeline">
          <ASRPipelinePage />
        </Route>
        <Route path="/dashboard/visualizations/fa-timeline">
          <FATimelinePage />
        </Route>
        <Route path="/dashboard/visualizations">
          <Layout><VisualizationsIndex /></Layout>
        </Route>

        <Route path="/dashboard/jobs/:id/traces/retokenize">
          <RetokenizationPage />
        </Route>
        <Route path="/dashboard/jobs/:id/traces/dp-alignment">
          <DPAlignmentPage />
        </Route>
        <Route path="/dashboard/jobs/:id/traces/asr-pipeline">
          <ASRPipelinePage />
        </Route>
        <Route path="/dashboard/jobs/:id/traces/fa-timeline">
          <FATimelinePage />
        </Route>

        <Route path="/dashboard/jobs/:id">
          <JobPage />
        </Route>
        <Route path="/dashboard/">
          <Layout>
            <DashboardPage />
          </Layout>
        </Route>
        <Route path="/dashboard">
          <Layout>
            <DashboardPage />
          </Layout>
        </Route>

        {/* Root: desktop defaults to /process, web defaults to /dashboard */}
        <Route path="/">
          {environment.isDesktop ? (
            <Redirect to="/process" />
          ) : (
            <Layout>
              <DashboardPage />
            </Layout>
          )}
        </Route>
      </Switch>
    </Router>
  );
}
