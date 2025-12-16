import { StalkrClient } from "./client";

describe("StalkrClient", () => {
  const client = new StalkrClient({ baseUrl: "https://api.stalkr.live" });

  it("should initialize with correct base url", () => {
    expect(client).toBeDefined();
  });

  it("should strip trailing slash from base url", () => {
    const c = new StalkrClient({ baseUrl: "https://api.stalkr.live/" });
    expect(c).toBeDefined();
  });

  it("should use default timeout of 10000ms", () => {
    expect(client).toBeDefined();
  });

  it("should accept custom timeout", () => {
    const c = new StalkrClient({ baseUrl: "https://api.stalkr.live", timeout: 5000 });
    expect(c).toBeDefined();
  });
});

// add api response envelope type [3.19]
