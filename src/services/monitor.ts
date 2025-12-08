import { logger } from "../utils/logger";

interface CityOverview {
  totalAgents: number;
  activeAgents: number;
  totalPipelines: number;
  activePipelines: number;
  totalRunsToday: number;
  successRate: number;
  uptimeHours: number;
}

interface AgentMetric {
  timestamp: string;
  cpuPercent: number;
  memoryMb: number;
  requestsPerMinute: number;
  errorRate: number;
}

interface CityStats {
  buildings: number;
  roads: number;
  population: number;
  happiness: number;
  revenue24h: number;
  costs24h: number;
}

export class MonitorService {
  private startTime = Date.now();

  async getOverview(): Promise<CityOverview> {
    return {
      totalAgents: 0,
      activeAgents: 0,
      totalPipelines: 0,
      activePipelines: 0,
      totalRunsToday: 0,
      successRate: 0,
      uptimeHours: (Date.now() - this.startTime) / 3_600_000,
    };
  }

  async getAgentMetrics(
    agentId: string,
    timeframe: string
  ): Promise<AgentMetric[]> {
    logger.debug({ agentId, timeframe }, "Fetching agent metrics");
    return [];
  }

  async getCityStats(): Promise<CityStats> {
    return {
      buildings: 0,
      roads: 0,
      population: 0,
      happiness: 100,
      revenue24h: 0,
      costs24h: 0,
    };
  }

  async getPipelineRuns(pipelineId: string, limit: number): Promise<unknown[]> {
    logger.debug({ pipelineId, limit }, "Fetching pipeline runs");
    return [];
  }
}
