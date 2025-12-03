import { pgTable, text, timestamp, jsonb, integer, real, varchar } from "drizzle-orm/pg-core";

export const agents = pgTable("agents", {
  id: varchar("id", { length: 24 }).primaryKey(),
  name: varchar("name", { length: 64 }).notNull(),
  type: varchar("type", { length: 16 }).notNull(),
  status: varchar("status", { length: 16 }).notNull().default("idle"),
  config: jsonb("config").notNull().default({}),
  description: text("description").default(""),
  totalRuns: integer("total_runs").notNull().default(0),
  successRate: real("success_rate").notNull().default(0),
  avgDurationMs: real("avg_duration_ms").notNull().default(0),
  lastRunAt: timestamp("last_run_at"),
  createdAt: timestamp("created_at").notNull().defaultNow(),
  updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export const pipelines = pgTable("pipelines", {
  id: varchar("id", { length: 24 }).primaryKey(),
  name: varchar("name", { length: 64 }).notNull(),
  steps: jsonb("steps").notNull().default([]),
  schedule: varchar("schedule", { length: 128 }),
  status: varchar("status", { length: 16 }).notNull().default("idle"),
  totalRuns: integer("total_runs").notNull().default(0),
  lastRunAt: timestamp("last_run_at"),
  createdAt: timestamp("created_at").notNull().defaultNow(),
  updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export const pipelineRuns = pgTable("pipeline_runs", {
  id: varchar("id", { length: 24 }).primaryKey(),
  pipelineId: varchar("pipeline_id", { length: 24 }).notNull(),
  status: varchar("status", { length: 16 }).notNull(),
  startedAt: timestamp("started_at").notNull().defaultNow(),
  completedAt: timestamp("completed_at"),
  stepResults: jsonb("step_results").notNull().default([]),
});

export const agentEvents = pgTable("agent_events", {
  id: varchar("id", { length: 24 }).primaryKey(),
  agentId: varchar("agent_id", { length: 24 }).notNull(),
  event: varchar("event", { length: 32 }).notNull(),
  data: jsonb("data").default({}),
  createdAt: timestamp("created_at").notNull().defaultNow(),
});
