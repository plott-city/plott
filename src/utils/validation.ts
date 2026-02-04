import { z } from "zod";

export function validateId(id: string): boolean {
  return /^[a-zA-Z0-9_-]{8,24}$/.test(id);
}

/** Validate and normalize pagination query params */
export function validatePagination(query: {
  page?: string;
  limit?: string;
}): { page: number; limit: number } {
  const page = Math.max(1, parseInt(query.page || "1", 10));
  const limit = Math.min(100, Math.max(1, parseInt(query.limit || "20", 10)));
  return { page, limit };
}

export const paginationSchema = z.object({
  page: z.coerce.number().min(1).default(1),
  limit: z.coerce.number().min(1).max(100).default(20),
});

export function sanitizeString(input: string): string {
  return input.replace(/[<>&"']/g, "").trim();
}

export function isValidCronExpression(expr: string): boolean {
  const parts = expr.split(" ");
  return parts.length === 5 || parts.length === 6;
}
