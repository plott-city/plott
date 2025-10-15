export const ErrorMessages = {
  AGENT_NOT_FOUND: "Agent not found",
  PIPELINE_NOT_FOUND: "Pipeline not found",
  INVALID_TOKEN: "Invalid token",
  MISSING_AUTH: "Missing or invalid authorization header",
  RATE_LIMITED: "Too many requests",
  INTERNAL_ERROR: "Internal server error",
  INVALID_INPUT: "Invalid input data",
} as const;

export class AppError extends Error {
  constructor(
    public statusCode: number,
    message: string
  ) {
    super(message);
    this.name = "AppError";
  }
}

export class NotFoundError extends AppError {
  constructor(resource: string) {
    super(404, `${resource} not found`);
    this.name = "NotFoundError";
  }
}

export class UnauthorizedError extends AppError {
  constructor(message = ErrorMessages.INVALID_TOKEN) {
    super(401, message);
    this.name = "UnauthorizedError";
  }
}
