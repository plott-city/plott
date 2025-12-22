import { beforeAll, afterAll } from "vitest";

beforeAll(async () => {
  // Setup test database or mocks
  process.env.NODE_ENV = "test";
  process.env.LOG_LEVEL = "silent";
});

afterAll(async () => {
  // Cleanup
});
