import { Context, Next } from "hono";
import { logger } from "../utils/logger";

/** Middleware that logs request duration */
export async function timingMiddleware(c: Context, next: Next) {
  const start = performance.now();
  await next();
  const durationMs = Math.round(performance.now() - start);

  logger.debug(
    {
      method: c.req.method,
      path: c.req.path,
      status: c.res.status,
      durationMs,
    },
    "Request completed"
  );

  c.header("X-Response-Time", `${durationMs}ms`);
}
