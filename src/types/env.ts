/**
 * Hono environment bindings for the Plott application.
 * Use with `new Hono<AppEnv>()` to get typed context variables.
 */
export interface AppEnv {
  Variables: {
    requestId: string;
    userId?: string;
    walletAddress?: string;
  };
}
