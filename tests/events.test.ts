import { describe, it, expect, vi } from "vitest";
import { EventBus } from "../src/utils/events";

describe("EventBus", () => {
  it("should call registered handlers", () => {
    const bus = new EventBus();
    const handler = vi.fn();
    bus.on("test", handler);
    bus.emit("test", "data");
    expect(handler).toHaveBeenCalledWith("data");
  });

  it("should remove handlers with off()", () => {
    const bus = new EventBus();
    const handler = vi.fn();
    bus.on("test", handler);
    bus.off("test", handler);
    bus.emit("test");
    expect(handler).not.toHaveBeenCalled();
  });

  it("should clear all handlers with removeAll()", () => {
    const bus = new EventBus();
    const handler = vi.fn();
    bus.on("a", handler);
    bus.on("b", handler);
    bus.removeAll();
    bus.emit("a");
    bus.emit("b");
    expect(handler).not.toHaveBeenCalled();
  });

  it("should not throw when handler errors", () => {
    const bus = new EventBus();
    bus.on("test", () => { throw new Error("fail"); });
    expect(() => bus.emit("test")).not.toThrow();
  });
});
