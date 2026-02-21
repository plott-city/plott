import { Hono } from "hono";
import { zValidator } from "@hono/zod-validator";
import { z } from "zod";
import { AgentRegistry } from "../services/agent-registry";
import { logger } from "../utils/logger";

export const agentRouter = new Hono();

const createAgentSchema = z.object({
  name: z.string().min(1).max(64),
  type: z.enum(["trading", "monitor", "executor", "custom"]),
  config: z.record(z.unknown()).optional(),
  description: z.string().max(256).optional(),
});

// Singleton agent registry instance
const registry = new AgentRegistry();

agentRouter.get("/", async (c) => {
  const agents = await registry.listAgents();
  return c.json({ agents, count: agents.length });
});

agentRouter.get("/:id", async (c) => {
  const id = c.req.param("id");
  const agent = await registry.getAgent(id);
  if (!agent) {
    return c.json({ error: "Agent not found" }, 404);
  }
  return c.json({ agent });
});

agentRouter.post("/", zValidator("json", createAgentSchema), async (c) => {
  const body = c.req.valid("json");
  logger.info({ name: body.name, type: body.type }, "Creating new agent");
  const agent = await registry.createAgent(body);
  return c.json({ agent }, 201);
});

agentRouter.put("/:id/start", async (c) => {
  const id = c.req.param("id");
  const agent = await registry.getAgent(id);
  if (!agent) return c.json({ error: "Agent not found" }, 404);
  await registry.startAgent(id);
  return c.json({ status: "started" });
});

agentRouter.put("/:id/stop", async (c) => {
  const id = c.req.param("id");
  const agent2 = await registry.getAgent(id);
  if (!agent2) return c.json({ error: "Agent not found" }, 404);
  await registry.stopAgent(id);
  return c.json({ status: "stopped" });
});

agentRouter.delete("/:id", async (c) => {
  const id = c.req.param("id");
  await registry.deleteAgent(id);
  return c.json({ status: "deleted" });
});
