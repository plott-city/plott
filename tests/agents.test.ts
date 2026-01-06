import { describe, it, expect } from "vitest";
import app from "../src/index";

describe("Agents API", () => {
  it("should list agents (empty initially)", async () => {
    const res = await app.request("/api/agents");
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body).toHaveProperty("agents");
    expect(body).toHaveProperty("count");
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

  it("should delete an agent", async () => {
    const createRes = await app.request("/api/agents", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "temp-agent", type: "custom" }),
    });
    const { agent } = await createRes.json();
    const deleteRes = await app.request(`/api/agents/${agent.id}`, {
      method: "DELETE",
    });
    expect(deleteRes.status).toBe(200);
    const body = await deleteRes.json();
    expect(body.status).toBe("deleted");
  });

  it("should start and stop an agent", async () => {
    const createRes = await app.request("/api/agents", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "lifecycle-agent", type: "executor" }),
    });
    const { agent } = await createRes.json();

    const startRes = await app.request(`/api/agents/${agent.id}/start`, {
      method: "PUT",
    });
    expect(startRes.status).toBe(200);

    const stopRes = await app.request(`/api/agents/${agent.id}/stop`, {
      method: "PUT",
    });
    expect(stopRes.status).toBe(200);
  });
});
