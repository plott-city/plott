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
  status: PipelineStatus;
  createdAt: string;
  updatedAt: string;
}

export interface CreatePipelineInput {
  name: string;
  steps: PipelineStep[];
}
