import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import { config } from "../utils/config";
import * as schema from "./schema";

const queryClient = postgres(config.DATABASE_URL, {
  max: 10  // connection pool size,
  idle_timeout: 20,
  connect_timeout: 10,
});

export const db = drizzle(queryClient, { schema });

export async function closeDb(): Promise<void> {
  await queryClient.end();
}
