/** Application-wide constants */

export const MAX_AGENT_NAME_LENGTH = 64;
export const MAX_DESCRIPTION_LENGTH = 256;
export const MAX_PIPELINE_STEPS = 20;
export const DEFAULT_PAGE_SIZE = 20;
export const MAX_PAGE_SIZE = 100;
export const RATE_LIMIT_WINDOW_MS = 60_000;
export const RATE_LIMIT_MAX_REQUESTS = 100;

/** Agent types supported by the platform */
export const AGENT_TYPES = ["trading", "monitor", "executor", "custom"] as const;

/** Pipeline status values */
export const PIPELINE_STATUSES = ["idle", "running", "failed", "completed"] as const;
