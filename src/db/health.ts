import { db } from "./client";
import { sql } from "drizzle-orm";

/** Check database connectivity by running a simple query */
export async function checkDbHealth(): Promise<boolean> {
  try {
    await db.execute(sql`SELECT 1`);
    return true;
  } catch {
    return false;
  }
}
