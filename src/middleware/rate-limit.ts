import { Context, Next } from "hono";

interface RateLimitConfig {
  maxRequests: number;
  windowMs: number;
}

const store = new Map<string, { count: number; resetAt: number }>();

export function rateLimiter(config: RateLimitConfig) {
  return async (c: Context, next: Next) => {
    const key = c.req.header("x-forwarded-for") || "unknown";
    const now = Date.now();

    let entry = store.get(key);
    if (!entry || entry.resetAt < now) {
      entry = { count: 0, resetAt: now + config.windowMs };
      store.set(key, entry);
    }

    entry.count += 1;

    if (entry.count > config.maxRequests) {
      return c.json({ error: "Too many requests" }, 429);
    }

    await next();
  };
}
