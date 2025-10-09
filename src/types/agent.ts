export type AgentType = "trading" | "monitor" | "executor" | "custom";

export type AgentStatus = "idle" | "running" | "error" | "stopped";

export interface Agent {
  id: string;
  name: string;
  type: AgentType;
  status: AgentStatus;
  config: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface CreateAgentInput {
  name: string;
  type: AgentType;
  config?: Record<string, unknown>;
}
