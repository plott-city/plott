import { serve } from "@hono/node-server";
import { Hono } from "hono";

const app = new Hono();

app.get("/", (c) => {
  return c.json({ name: "plott", version: "0.1.0" });
});

const port = parseInt(process.env.PORT || "3001");

serve({ fetch: app.fetch, port }, (info) => {
  console.log(`Server running on http://localhost:${info.port}`);
});

export default app;
