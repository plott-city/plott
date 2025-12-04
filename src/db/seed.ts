import { db } from "./client";
import { agents } from "./schema";
import { nanoid } from "nanoid";
import { logger } from "../utils/logger";

/** Seed the database with sample agents for development */
export async function seed(): Promise<void> {
  const sampleAgents = [
    { name: "price-watcher", type: "monitor", description: "Watches SOL/USDC price feed" },
    { name: "trade-executor", type: "executor", description: "Executes limit orders" },
    { name: "arb-scanner", type: "trading", description: "Scans for arbitrage opportunities" },
  ];

  for (const agent of sampleAgents) {
    const id = nanoid(12);
    await db.insert(agents).values({
      id,
      name: agent.name,
      type: agent.type,
      description: agent.description,
      status: "idle",
      config: {},
    });
    logger.info({ id, name: agent.name }, "Seeded agent");
  }

  logger.info(`Seeded ${sampleAgents.length} agents`);
}

if (require.main === module) {
  seed()
    .then(() => process.exit(0))
    .catch((err) => {
      logger.error({ err }, "Seed failed");
      process.exit(1);
    });
}
