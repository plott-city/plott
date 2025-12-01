import { nanoid } from "nanoid";
import { logger } from "../utils/logger";
import type { Agent, AgentStatus, CreateAgentInput } from "../types/agent";

export class AgentRegistry {
  private agents: Map<string, Agent> = new Map();

  async listAgents(): Promise<Agent[]> {
    return Array.from(this.agents.values());
  }

  async getAgent(id: string): Promise<Agent | undefined> {
    return this.agents.get(id);
  }

  async createAgent(input: CreateAgentInput): Promise<Agent> {
    const id = nanoid(12);
    const now = new Date().toISOString();

    const agent: Agent = {
      id,
      name: input.name,
      type: input.type,
      status: "idle",
      config: input.config || {},
      description: input.description || "",
      metrics: {
        totalRuns: 0,
        successRate: 0,
        avgDurationMs: 0,
        lastRunAt: null,
      },
      createdAt: now,
      updatedAt: now,
    };

    this.agents.set(id, agent);
    logger.info({ agentId: id, name: input.name }, "Agent registered");
    return agent;
  }

  async startAgent(id: string): Promise<void> {
    const agent = this.agents.get(id);
    if (!agent) throw new Error(`Agent ${id} not found`);
    agent.status = "running";
    agent.updatedAt = new Date().toISOString();
    logger.info({ agentId: id }, "Agent started");
  }

  async stopAgent(id: string): Promise<void> {
    const agent = this.agents.get(id);
    if (!agent) throw new Error(`Agent ${id} not found`);
    agent.status = "idle";
    agent.updatedAt = new Date().toISOString();
    logger.info({ agentId: id }, "Agent stopped");
  }

  async deleteAgent(id: string): Promise<void> {
    const existed = this.agents.delete(id);
    if (!existed) throw new Error(`Agent ${id} not found`);
    logger.info({ agentId: id }, "Agent deleted");
  }

  async updateStatus(id: string, status: AgentStatus): Promise<void> {
    const agent = this.agents.get(id);
    if (!agent) throw new Error(`Agent ${id} not found`);
    agent.status = status;
    agent.updatedAt = new Date().toISOString();
  }
}
