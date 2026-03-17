import { db } from "./client";
import { agents, pipelines, pipelineRuns } from "./schema";
import { eq, desc } from "drizzle-orm";

/** Find agent by primary key */
export async function findAgentById(id: string) {
  const result = await db.select().from(agents).where(eq(agents.id, id)).limit(1);
  return result[0] || null;
}

export async function findAllAgents() {
  return db.select().from(agents);
}

export async function findPipelineById(id: string) {
  const result = await db.select().from(pipelines).where(eq(pipelines.id, id)).limit(1);
  return result[0] || null;
}

export async function findRecentRuns(pipelineId: string, limit: number) {
  return db
    .select()
    .from(pipelineRuns)
    .where(eq(pipelineRuns.pipelineId, pipelineId))
    .orderBy(desc(pipelineRuns.startedAt))
    .limit(limit);
}
