import { describe, it, expect } from "vitest";
import app from "../src/index";

describe("Agents API", () => {
  it("should list agents (empty initially)", async () => {
    const res = await app.request("/api/agents");
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.agents).toEqual([]);
    expect(body.count).toBe(0);
  });

  it("should create a new agent", async () => {
    const res = await app.request("/api/agents", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "price-tracker",
        type: "monitor",
        description: "Tracks SOL/USDC price feed",
      }),
    });
    expect(res.status).toBe(201);
    const body = await res.json();
    expect(body.agent.name).toBe("price-tracker");
    expect(body.agent.type).toBe("monitor");
    expect(body.agent.status).toBe("idle");
  });

  it("should reject invalid agent type", async () => {
    const res = await app.request("/api/agents", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "bad-agent",
        type: "invalid",
      }),
    });
    expect(res.status).toBe(400);
  });

  it("should return 404 for non-existent agent", async () => {
    const res = await app.request("/api/agents/nonexistent123");
    expect(res.status).toBe(404);
  });
});
