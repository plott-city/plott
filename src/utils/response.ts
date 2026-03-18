import { Context } from "hono";

export function success<T>(c: Context, data: T, status: 200 | 201 = 200) {
  return c.json(data, status);
}

export function error(c: Context, message: string, status: number = 500) {
  return c.json({ error: message }, status as 400 | 401 | 403 | 404 | 500);
}

export function notFound(c: Context, resource: string) {
  return c.json({ error: `${resource} not found` }, 404);
}

export function paginated<T>(c: Context, items: T[], total: number, page: number, limit: number) {
  return c.json({
    items,
    pagination: {
      total,
      page,
      limit,
      totalPages: Math.ceil(total / limit),
    },
  });
}
