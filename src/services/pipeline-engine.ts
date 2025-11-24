import { nanoid } from "nanoid";
import { logger } from "../utils/logger";
import type { Pipeline, CreatePipelineInput } from "../types/pipeline";

export class PipelineEngine {
  private pipelines: Map<string, Pipeline> = new Map();

  async listPipelines(): Promise<Pipeline[]> {
    return Array.from(this.pipelines.values());
  }

  async getPipeline(id: string): Promise<Pipeline | undefined> {
    return this.pipelines.get(id);
  }

  async createPipeline(input: CreatePipelineInput): Promise<Pipeline> {
    const id = nanoid(12);
    const now = new Date().toISOString();

    const pipeline: Pipeline = {
      id,
      name: input.name,
      steps: input.steps,
      status: "idle",
      createdAt: now,
      updatedAt: now,
    };

    this.pipelines.set(id, pipeline);
    logger.info({ pipelineId: id, name: input.name }, "Pipeline created");
    return pipeline;
  }

  async deletePipeline(id: string): Promise<void> {
    const existed = this.pipelines.delete(id);
    if (!existed) throw new Error(`Pipeline ${id} not found`);
    logger.info({ pipelineId: id }, "Pipeline deleted");
  }
}
