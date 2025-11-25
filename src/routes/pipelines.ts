import { Hono } from "hono";
import { z } from "zod";
import { zValidator } from "@hono/zod-validator";
import { PipelineEngine } from "../services/pipeline-engine";
import { logger } from "../utils/logger";

export const pipelineRouter = new Hono();

const createPipelineSchema = z.object({
  name: z.string().min(1).max(64),
  steps: z.array(
    z.object({
      agentId: z.string(),
      action: z.string(),
      params: z.record(z.unknown()).optional(),
    })
  ),
  schedule: z.string().optional(),
});

const engine = new PipelineEngine();

pipelineRouter.get("/", async (c) => {
  const pipelines = await engine.listPipelines();
  return c.json({ pipelines });
});

pipelineRouter.get("/:id", async (c) => {
  const id = c.req.param("id");
  const pipeline = await engine.getPipeline(id);
  if (!pipeline) {
    return c.json({ error: "Pipeline not found" }, 404);
  }
  return c.json({ pipeline });
});

pipelineRouter.post("/", zValidator("json", createPipelineSchema), async (c) => {
  const body = c.req.valid("json");
  logger.info({ name: body.name, steps: body.steps.length }, "Creating pipeline");
  const pipeline = await engine.createPipeline(body);
  return c.json({ pipeline }, 201);
});

pipelineRouter.put("/:id/trigger", async (c) => {
  const id = c.req.param("id");
  const pipeline = await engine.getPipeline(id);
  if (!pipeline) return c.json({ error: "Pipeline not found" }, 404);
  // Trigger and track the pipeline run
  const run = await engine.triggerPipeline(id);
  return c.json({ run });
});

pipelineRouter.delete("/:id", async (c) => {
  const id = c.req.param("id");
  await engine.deletePipeline(id);
  return c.json({ status: "deleted" });
});
