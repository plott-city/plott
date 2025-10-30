import { drizzle } from "drizzle-orm/postgres-js";
import { migrate } from "drizzle-orm/postgres-js/migrator";
import postgres from "postgres";
import { config } from "../utils/config";
import { logger } from "../utils/logger";

export async function runMigrations(): Promise<void> {
  const sql = postgres(config.DATABASE_URL, { max: 1 });
  const db = drizzle(sql);

  logger.info("Running database migrations...");

  try {
    await migrate(db, { migrationsFolder: "./drizzle" });
    logger.info("Migrations completed successfully");
  } catch (err) {
    logger.error({ err }, "Migration failed");
    throw err;
  } finally {
    await sql.end();
  }
}

if (require.main === module) {
  runMigrations()
    .then(() => process.exit(0))
    .catch(() => process.exit(1));
}
