import { pgTable, text, timestamp, jsonb, varchar } from "drizzle-orm/pg-core";

export const agents = pgTable("agents", {
  id: varchar("id", { length: 24 }).primaryKey(),
  name: varchar("name", { length: 64 }).notNull(),
  type: varchar("type", { length: 16 }).notNull(),
  status: varchar("status", { length: 16 }).notNull().default("idle"),
  config: jsonb("config").notNull().default({}),
  createdAt: timestamp("created_at").notNull().defaultNow(),
  updatedAt: timestamp("updated_at").notNull().defaultNow(),
});
