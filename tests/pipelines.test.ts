import { describe, it, expect } from "vitest";
import app from "../src/index";

describe("Pipelines API", () => {
  it("should list pipelines", async () => {
    const res = await app.request("/api/pipelines");
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.pipelines).toEqual([]);
  });

  it("should create a pipeline", async () => {
    const res = await app.request("/api/pipelines", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "price-to-trade",
        steps: [
          { agentId: "agent1", action: "fetchPrice" },
          { agentId: "agent2", action: "executeTrade", params: { pair: "SOL/USDC" } },
        ],
      }),
    });
    expect(res.status).toBe(201);
    const body = await res.json();
    expect(body.pipeline.name).toBe("price-to-trade");
    expect(body.pipeline.steps).toHaveLength(2);
  });

  it("should return 404 for non-existent pipeline", async () => {
    const res = await app.request("/api/pipelines/nonexistent123");
    expect(res.status).toBe(404);
  });
});
