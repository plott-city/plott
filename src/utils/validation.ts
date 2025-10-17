import { z } from "zod";

export function validateId(id: string): boolean {
  return /^[a-zA-Z0-9_-]{8,24}$/.test(id);
}

export function validatePagination(query: {
  page?: string;
  limit?: string;
}): { page: number; limit: number } {
  const page = Math.max(1, parseInt(query.page || "1", 10));
  const limit = Math.min(100, Math.max(1, parseInt(query.limit || "20", 10)));
  return { page, limit };
}
