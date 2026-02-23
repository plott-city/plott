import { nanoid } from "nanoid";
import { logger } from "../utils/logger";
import type {
  Pipeline,
  PipelineRun,
  PipelineStep,
  CreatePipelineInput,
} from "../types/pipeline";

export class PipelineEngine {
  private pipelines: Map<string, Pipeline> = new Map();
  private runs: Map<string, PipelineRun[]> = new Map();

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
      schedule: input.schedule || null,
      status: "idle",
      totalRuns: 0,
      lastRunAt: null,
      createdAt: now,
      updatedAt: now,
    };

    this.pipelines.set(id, pipeline);
    this.runs.set(id, []);
    logger.info({ pipelineId: id, name: input.name }, "Pipeline created");
    return pipeline;
  }

  async triggerPipeline(id: string): Promise<PipelineRun> {
    const pipeline = this.pipelines.get(id);
    if (!pipeline) throw new Error(`Pipeline ${id} not found`);

    const runId = nanoid(12);
    const now = new Date().toISOString();

    const run: PipelineRun = {
      id: runId,
      pipelineId: id,
      status: "running",
      startedAt: now,
      completedAt: null,
      stepResults: [],
    };

    pipeline.totalRuns += 1;
    pipeline.lastRunAt = now;
    pipeline.status = "running";
    pipeline.updatedAt = now;

    const pipelineRuns = this.runs.get(id) || [];
    pipelineRuns.push(run);
    this.runs.set(id, pipelineRuns);

    logger.info({ pipelineId: id, runId }, "Pipeline triggered");

    // Execute steps sequentially
    this.executeSteps(pipeline, run).catch((err) => {
      logger.error({ pipelineId: id, runId, err }, "Pipeline execution failed");
      run.status = "failed";
      run.completedAt = new Date().toISOString();
    });

    return run;
  }

  private async executeSteps(pipeline: Pipeline, run: PipelineRun): Promise<void> {
    for (const step of pipeline.steps) {
      const stepStart = Date.now();
      try {
        // Step execution would go here
        logger.info(
          { pipelineId: pipeline.id, agentId: step.agentId, action: step.action },
          "Executing pipeline step"
        );
        run.stepResults.push({
          agentId: step.agentId,
          action: step.action,
          status: "completed",
          durationMs: Date.now() - stepStart,
        });
      } catch (err) {
        run.stepResults.push({
          agentId: step.agentId,
          action: step.action,
          status: "failed",
          durationMs: Date.now() - stepStart,
        });
        throw err;
      }
    }
    run.status = "completed";
    run.completedAt = new Date().toISOString();
    pipeline.status = "idle";
  }

  async deletePipeline(id: string): Promise<void> {
    const existed = this.pipelines.delete(id);
    this.runs.delete(id);
    if (!existed) throw new Error(`Pipeline ${id} not found`);
    logger.info({ pipelineId: id }, "Pipeline deleted");
  }

  async getPipelineRuns(id: string, limit: number = 20): Promise<PipelineRun[]> {
    const runs = this.runs.get(id) || [];
    return runs.slice(-limit);
  }
}
