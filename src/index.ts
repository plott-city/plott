import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { logger as honoLogger } from "hono/logger";
import { cors } from "hono/cors";
import { healthRouter } from "./routes/health";
import { agentRouter } from "./routes/agents";
import { pipelineRouter } from "./routes/pipelines";
import { dashboardRouter } from "./routes/dashboard";
// Auth middleware
import { authMiddleware } from "./middleware/auth";
import { rateLimiter } from "./middleware/rate-limit";
import { logger } from "./utils/logger";
import { config } from "./utils/config";

const app = new Hono();

// Global middleware
app.use("*", honoLogger());
app.use("*", cors({ origin: config.CORS_ORIGINS }));

// Public routes
app.route("/health", healthRouter);

// Protected routes
app.use("/api/*", rateLimiter({ maxRequests: 100, windowMs: 60_000 }));
app.route("/api/agents", agentRouter);
app.route("/api/pipelines", pipelineRouter);
app.route("/api/dashboard", dashboardRouter);

app.get("/", (c) => {
  return c.json({
    name: "plott",
    version: "0.4.0",
    message: "Build your agent city.",
  });
});

// Error handler
app.onError((err, c) => {
  logger.error({ err: err.message, path: c.req.path }, "Unhandled error");
  return c.json({ error: "Internal server error" }, 500);
});

// Only start server when run directly (not imported by tests)
if (process.env.NODE_ENV !== "test") {
  const port = config.PORT;
  serve({ fetch: app.fetch, port }, (info) => {
    logger.info(`Plott server running on http://localhost:${info.port}`);
  });

  process.on("SIGTERM", () => {
    logger.info("SIGTERM received, shutting down");
    process.exit(0);
  });
}

export default app;
