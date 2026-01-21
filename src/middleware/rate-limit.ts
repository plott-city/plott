import { Context, Next } from "hono";

interface RateLimitConfig {
  maxRequests: number;
  windowMs: number;
}

interface RateLimitEntry {
  count: number;
  resetAt: number;
}

const store = new Map<string, RateLimitEntry>();

// Cleanup stale entries every 5 minutes
const cleanupInterval = setInterval(() => {
  const now = Date.now();
  for (const [key, entry] of store.entries()) {
    if (entry.resetAt < now) {
      store.delete(key);
    }
  }
}, 300_000);
cleanupInterval.unref();

/** Manually clear all rate limit entries (for testing) */
export function clearRateLimitStore() {
  store.clear();
}

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

    c.header("X-RateLimit-Limit", config.maxRequests.toString());
    c.header("X-RateLimit-Remaining", Math.max(0, config.maxRequests - entry.count).toString());
    c.header("X-RateLimit-Reset", new Date(entry.resetAt).toISOString());

    if (entry.count > config.maxRequests) {
      return c.json({ error: "Too many requests" }, 429);
    }

    await next();
  };
}
