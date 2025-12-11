import { Context, Next } from "hono";
import { config } from "../utils/config";

/**
 * Custom CORS handler for fine-grained control.
 * For most cases, use hono/cors built-in middleware instead.
 */
export async function corsMiddleware(c: Context, next: Next) {
  const origin = c.req.header("Origin");
  const allowedOrigins = config.CORS_ORIGINS;

  if (origin && allowedOrigins.includes(origin)) {
    c.header("Access-Control-Allow-Origin", origin);
    c.header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    c.header("Access-Control-Allow-Headers", "Content-Type, Authorization");
    c.header("Access-Control-Max-Age", "86400");
  }

  if (c.req.method === "OPTIONS") {
    return new Response(null, { status: 204 });
  }

  await next();
}
