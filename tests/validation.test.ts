import { describe, it, expect } from "vitest";
import { validateId, validatePagination, sanitizeString, isValidCronExpression } from "../src/utils/validation";

describe("Validation utilities", () => {
  describe("validateId", () => {
    it("should accept valid IDs", () => {
      expect(validateId("abc123def456")).toBe(true);
      expect(validateId("a1b2c3d4e5f6g7h8")).toBe(true);
    });

    it("should reject short IDs", () => {
      expect(validateId("abc")).toBe(false);
    });

    it("should reject IDs with special characters", () => {
      expect(validateId("abc!@#$%^&*()")).toBe(false);
    });
  });

  describe("validatePagination", () => {
    it("should return defaults for empty query", () => {
      const result = validatePagination({});
      expect(result.page).toBe(1);
      expect(result.limit).toBe(20);
    });

    it("should clamp limit to 100", () => {
      const result = validatePagination({ limit: "500" });
      expect(result.limit).toBe(100);
    });

    it("should enforce minimum page of 1", () => {
      const result = validatePagination({ page: "-5" });
      expect(result.page).toBe(1);
    });
  });

  describe("sanitizeString", () => {
    it("should remove HTML characters", () => {
      expect(sanitizeString("<script>alert(1)</script>")).not.toContain("<");
    });

    it("should trim whitespace", () => {
      expect(sanitizeString("  hello  ")).toBe("hello");
    });
  });

  describe("isValidCronExpression", () => {
    it("should accept 5-part cron", () => {
      expect(isValidCronExpression("*/5 * * * *")).toBe(true);
    });

    it("should reject invalid cron", () => {
      expect(isValidCronExpression("not a cron")).toBe(false);
    });
  });
});
