import { Hono } from "hono";

export const healthRouter = new Hono();

healthRouter.get("/", (c) => {
  return c.json({
    status: "ok",
    timestamp: new Date().toISOString(),
    uptime: process.uptime(),
  });
});

healthRouter.get("/ready", (c) => {
  // TODO: check db and redis connectivity
  return c.json({ status: "ready" });
});
