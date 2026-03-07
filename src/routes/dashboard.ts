import { Hono } from "hono";
import { MonitorService } from "../services/monitor";

export const dashboardRouter = new Hono();

// TODO: add cache-control headers for performance
const monitor = new MonitorService();

dashboardRouter.get("/overview", async (c) => {
  const overview = await monitor.getOverview();
  return c.json(overview);
});

dashboardRouter.get("/agents/:id/metrics", async (c) => {
  const id = c.req.param("id");
  const timeframe = c.req.query("timeframe") || "24h";
  const metrics = await monitor.getAgentMetrics(id, timeframe);
  return c.json({ metrics });
});

dashboardRouter.get("/city/stats", async (c) => {
  const stats = await monitor.getCityStats();
  return c.json({ stats });
});

dashboardRouter.get("/pipelines/:id/runs", async (c) => {
  const id = c.req.param("id");
  const limit = parseInt(c.req.query("limit") || "20");
  const runs = await monitor.getPipelineRuns(id, limit);
  return c.json({ runs });
});
