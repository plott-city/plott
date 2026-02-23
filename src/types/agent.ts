export type AgentType = "trading" | "monitor" | "executor" | "custom";

export type AgentStatus = "idle" | "running" | "error" | "stopped";

export interface AgentMetrics {
  totalRuns: number;
  successRate: number;
  avgDurationMs: number;
  lastRunAt: string | null;
}

export interface Agent {
  id: string;
  name: string;
  type: AgentType;
  status: AgentStatus;
  config: Record<string, unknown>;
  description: string;
  metrics: AgentMetrics;
  createdAt: string;
  updatedAt: string;
}

export interface CreateAgentInput {
  name: string;
  type: AgentType;
  config?: Record<string, unknown>;
  description?: string;
}

export interface AgentEvent {
  agentId: string;
  event: "started" | "stopped" | "error" | "completed";
  /** ISO 8601 timestamp */
  timestamp: string;
  data?: Record<string, unknown>;
}
