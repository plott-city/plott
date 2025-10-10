import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { logger as honoLogger } from "hono/logger";
import { healthRouter } from "./routes/health";
import { logger } from "./utils/logger";
import { config } from "./utils/config";

const app = new Hono();

app.use("*", honoLogger());

app.route("/health", healthRouter);

app.get("/", (c) => {
  return c.json({
    name: "plott",
    version: "0.2.0",
    message: "Agent orchestration platform",
  });
});

app.onError((err, c) => {
  logger.error({ err: err.message, path: c.req.path }, "Unhandled error");
  return c.json({ error: "Internal server error" }, 500);
});

const port = config.PORT;

serve({ fetch: app.fetch, port }, (info) => {
  logger.info(`Plott server running on http://localhost:${info.port}`);
});

export default app;
