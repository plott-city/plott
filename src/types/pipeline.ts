export type PipelineStatus = "idle" | "running" | "failed" | "completed";

export interface PipelineStep {
  agentId: string;
  action: string;
  params?: Record<string, unknown>;
}

export interface Pipeline {
  id: string;
  name: string;
  steps: PipelineStep[];
  schedule: string | null;
  status: PipelineStatus;
  totalRuns: number;
  lastRunAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreatePipelineInput {
  name: string;
  steps: PipelineStep[];
  schedule?: string;
}

export interface StepResult {
  agentId: string;
  action: string;
  status: "completed" | "failed" | "skipped";
  durationMs: number;
  output?: unknown;
}

export interface PipelineRun {
  id: string;
  pipelineId: string;
  status: PipelineStatus;
  startedAt: string;
  completedAt: string | null;
  stepResults: StepResult[];
}
