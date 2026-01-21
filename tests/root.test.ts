import { describe, it, expect } from "vitest";
import app from "../src/index";

describe("Root endpoint", () => {
  it("should return app info", async () => {
    const res = await app.request("/");
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.name).toBe("plott");
    expect(body).toHaveProperty("version");
    expect(body).toHaveProperty("message");
  });
});
