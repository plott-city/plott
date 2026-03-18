import { logger } from "./logger";
import { closeDb } from "../db/client";

const shutdownHandlers: Array<() => Promise<void>> = [];

/** Register a handler to run during graceful shutdown */
export function onShutdown(handler: () => Promise<void>): void {
  shutdownHandlers.push(handler);
}

/** Execute all shutdown handlers in order */
export async function gracefulShutdown(signal: string): Promise<void> {
  logger.info({ signal }, "Shutdown signal received");

  for (const handler of shutdownHandlers) {
    try {
      await handler();
    } catch (err) {
      logger.error({ err }, "Shutdown handler failed");
    }
  }

  try {
    await closeDb();
    logger.info("Database connections closed");
  } catch (err) {
    logger.error({ err }, "Failed to close database");
  }

  process.exit(0);
}

// Register signal handlers
process.on("SIGTERM", () => gracefulShutdown("SIGTERM"));
process.on("SIGINT", () => gracefulShutdown("SIGINT"));
