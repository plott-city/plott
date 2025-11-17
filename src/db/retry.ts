import { logger } from "../utils/logger";

const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 1000;

/** Retry a database operation with exponential backoff */
export async function withRetry<T>(
  operation: () => Promise<T>,
  label: string = "db operation"
): Promise<T> {
  let lastError: Error | undefined;

  for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
    try {
      return await operation();
    } catch (err) {
      lastError = err as Error;
      logger.warn(
        { attempt, maxRetries: MAX_RETRIES, error: lastError.message },
        `${label} failed, retrying...`
      );
      if (attempt < MAX_RETRIES) {
        await new Promise((r) => setTimeout(r, RETRY_DELAY_MS * attempt));
      }
    }
  }

  throw lastError;
}
