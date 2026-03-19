<p align="center">
  <img src="assets/banner.png" alt="Plott - Build your agent city" width="100%" />
</p>

<p align="center">
  <strong>Build your agent city.</strong>
</p>
<p align="center">
  <strong>CA: EtuvAk4KhYCaYMfEJViNBm6gvaLkJm1XUzTjMFZxpump</strong>
</p>

<p align="center">
  <a href="https://plott.city/city">
    <img src="https://img.shields.io/badge/Web%20(Play)-4CAF50?style=for-the-badge&logo=googlechrome&logoColor=white" alt="Web (Play)" />
  </a>
  <a href="https://x.com/plottcity">
    <img src="https://img.shields.io/badge/X-000000?style=for-the-badge&logo=x&logoColor=white" alt="X" />
  </a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Hono-E36002?style=flat-square&logo=hono&logoColor=white" alt="Hono" />
  <img src="https://img.shields.io/badge/Solana-9945FF?style=flat-square&logo=solana&logoColor=white" alt="Solana" />
  <img src="https://img.shields.io/badge/PostgreSQL-4169E1?style=flat-square&logo=postgresql&logoColor=white" alt="PostgreSQL" />
  <img src="https://img.shields.io/badge/Redis-DC382D?style=flat-square&logo=redis&logoColor=white" alt="Redis" />
  <img src="https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white" alt="Docker" />
  <img src="https://img.shields.io/badge/Node.js-20+-339933?style=flat-square&logo=node.js&logoColor=white" alt="Node.js" />
  <a href="https://github.com/plott-city/plott/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/plott-city/plott/ci.yml?branch=main&style=flat-square&label=CI" alt="CI" />
  </a>
  <img src="https://img.shields.io/badge/License-MIT-yellow?style=flat-square" alt="License" />
</p>

---

## What is Plott?

Plott is an **agent orchestration platform** that lets you deploy, monitor, and connect autonomous agents like buildings in a city. Each agent is a building. Each pipeline is a road. You are the mayor.

- Deploy agents as modular, isolated units
- Connect agents through configurable pipelines
- Monitor everything from a real-time dashboard
- Scale from a single agent to an entire city

## Architecture

```mermaid
graph TD
    A[Client] -->|REST API| B[Hono Server]
    B --> C[Agent Registry]
    B --> D[Pipeline Engine]
    B --> E[Monitor Service]
    C --> F[(PostgreSQL)]
    D --> G[(Redis + BullMQ)]
    E --> F
    D --> C
    C -->|Events| H[WebSocket]
    H --> A

    style A fill:#64B5F6,stroke:#333,color:#000
    style B fill:#4CAF50,stroke:#333,color:#000
    style C fill:#FFB74D,stroke:#333,color:#000
    style D fill:#FFB74D,stroke:#333,color:#000
    style E fill:#FFB74D,stroke:#333,color:#000
    style F fill:#0F1923,stroke:#4CAF50,color:#F5ECD7
    style G fill:#0F1923,stroke:#4CAF50,color:#F5ECD7
    style H fill:#64B5F6,stroke:#333,color:#000
```

## Quick Start

### Prerequisites

- Node.js 20+
- PostgreSQL 16+
- Redis 7+

### Installation

```bash
git clone https://github.com/plott-city/plott.git
cd plott
npm install
cp .env.example .env
```

### Development

```bash
# Start infrastructure
docker compose up -d postgres redis

# Run development server
npm run dev
```

### Using Docker

```bash
docker compose up -d
```

The server starts at `http://localhost:3001`.

## API Overview

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/api/agents` | List all agents |
| POST | `/api/agents` | Register new agent |
| PUT | `/api/agents/:id/start` | Start an agent |
| PUT | `/api/agents/:id/stop` | Stop an agent |
| GET | `/api/pipelines` | List pipelines |
| POST | `/api/pipelines` | Create pipeline |
| PUT | `/api/pipelines/:id/trigger` | Trigger pipeline run |
| GET | `/api/dashboard/overview` | City overview stats |

## Project Structure

```
src/
  index.ts              # Hono app entry
  routes/
    health.ts           # Health checks
    agents.ts           # Agent CRUD
    pipelines.ts        # Pipeline management
    dashboard.ts        # Dashboard data
  services/
    agent-registry.ts   # Agent lifecycle management
    pipeline-engine.ts  # Pipeline execution engine
    monitor.ts          # Metrics and monitoring
  middleware/
    auth.ts             # Authentication
    rate-limit.ts       # Rate limiting
    cors.ts             # CORS configuration
  db/
    schema.ts           # Database schema (Drizzle)
    client.ts           # Database client
    migrations.ts       # Migration runner
  types/
    agent.ts            # Agent type definitions
    pipeline.ts         # Pipeline type definitions
  utils/
    logger.ts           # Pino logger
    config.ts           # Environment config
    validation.ts       # Input validation
```

## Testing

```bash
npm test
npm run test:watch
npm run test:coverage
```

## License

MIT

---

<p align="center">
  <sub>Build your agent city.</sub>
</p>
