import { Hono } from "hono";
import { z } from "zod";
import { logger } from "../utils/logger";

export const agentRouter = new Hono();

const createAgentSchema = z.object({
  name: z.string().min(1).max(64),
  type: z.enum(["trading", "monitor", "executor", "custom"]),
  config: z.record(z.unknown()).optional(),
});

// In-memory store for now
const agents = new Map();

agentRouter.get("/", async (c) => {
  const all = Array.from(agents.values());
  return c.json({ agents: all, count: all.length });
});

agentRouter.get("/:id", async (c) => {
  const id = c.req.param("id");
  const agent = agents.get(id);
  if (!agent) {
    return c.json({ error: "Agent not found" }, 404);
  }
  return c.json({ agent });
});

agentRouter.post("/", async (c) => {
  const body = await c.req.json();
  const result = createAgentSchema.safeParse(body);
  if (!result.success) {
    return c.json({ error: result.error.flatten() }, 400);
  }
  const id = crypto.randomUUID().slice(0, 12);
  const agent = { id, ...result.data, status: "idle", createdAt: new Date().toISOString() };
  agents.set(id, agent);
  logger.info({ name: agent.name }, "Agent created");
  return c.json({ agent }, 201);
});
