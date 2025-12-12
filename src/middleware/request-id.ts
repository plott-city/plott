import { Context, Next } from "hono";
import { nanoid } from "nanoid";

export async function requestIdMiddleware(c: Context, next: Next) {
  const requestId = c.req.header("X-Request-ID") || nanoid(16);
  c.set("requestId", requestId);
  c.header("X-Request-ID", requestId);
  await next();
}
