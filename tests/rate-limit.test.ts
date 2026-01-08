import { describe, it, expect } from "vitest";
import { Hono } from "hono";
import { rateLimiter } from "../src/middleware/rate-limit";

describe("Rate limiter middleware", () => {
  it("should allow requests under the limit", async () => {
    const app = new Hono();
    app.use("*", rateLimiter({ maxRequests: 5, windowMs: 60_000 }));
    app.get("/test", (c) => c.json({ ok: true }));

    const res = await app.request("/test");
    expect(res.status).toBe(200);
    expect(res.headers.get("X-RateLimit-Limit")).toBe("5");
  });

  it("should include rate limit headers", async () => {
    const app = new Hono();
    app.use("*", rateLimiter({ maxRequests: 10, windowMs: 60_000 }));
    app.get("/test", (c) => c.json({ ok: true }));

    const res = await app.request("/test");
    expect(res.headers.has("X-RateLimit-Limit")).toBe(true);
    expect(res.headers.has("X-RateLimit-Remaining")).toBe(true);
    expect(res.headers.has("X-RateLimit-Reset")).toBe(true);
  });
});
