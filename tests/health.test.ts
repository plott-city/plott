import { describe, it, expect } from "vitest";
import app from "../src/index";

describe("Health endpoint", () => {
  it("should return status ok", async () => {
    const res = await app.request("/health");
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.status).toBe("ok");
    expect(body).toHaveProperty("timestamp");
    expect(body).toHaveProperty("uptime");
  });

  it("should return ready status", async () => {
    const res = await app.request("/health/ready");
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.status).toBe("ready");
  });
});
