import { Context, Next } from "hono";
import { logger } from "../utils/logger";

export async function authMiddleware(c: Context, next: Next) {
  const authHeader = c.req.header("Authorization");

  if (!authHeader || !authHeader.startsWith("Bearer ")) {
    return c.json({ error: "Missing or invalid authorization header" }, 401);
  }

  const token = authHeader.slice(7);

  try {
    // Verify token (wallet signature verification would go here)
    const payload = decodeToken(token);
    c.set("userId", payload.sub);
    c.set("walletAddress", payload.wallet);
    await next();
  } catch (err) {
    logger.warn({ err }, "Authentication failed");
    return c.json({ error: "Invalid token" }, 401);
  }
}

function decodeToken(token: string): { sub: string; wallet: string } {
  // Placeholder for actual JWT / wallet signature verification
  const parts = token.split(".");
  if (parts.length !== 3) throw new Error("Malformed token");
  const payload = JSON.parse(Buffer.from(parts[1], "base64url").toString());
  return payload;
}
