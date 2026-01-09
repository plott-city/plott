<div align="center">

<img src="./github-banner.png" alt="Stalkr" width="100%" />

# STALKR

### Predict the whale.

<a href="https://stalkr.live">
  <img src="https://img.shields.io/badge/Website-stalkr.live-0A1628?style=for-the-badge&logo=google-chrome&logoColor=D4A843" alt="Website" />
</a>
&nbsp;
<a href="https://x.com/stalkr_live">
  <img src="https://img.shields.io/badge/Twitter-@stalkr__live-0A1628?style=for-the-badge&logo=x&logoColor=D4A843" alt="Twitter" />
</a>
&nbsp;
<a href="LICENSE">
  <img src="https://img.shields.io/badge/License-MIT-0A1628?style=for-the-badge&logoColor=D4A843" alt="License" />
</a>

<br />

<a href="#">
  <img src="https://img.shields.io/badge/Rust-1.75-0A1628?style=flat-square&logo=rust&logoColor=D4A843" alt="Rust" />
</a>
&nbsp;
<a href="#">
  <img src="https://img.shields.io/badge/TypeScript-5.3-0A1628?style=flat-square&logo=typescript&logoColor=D4A843" alt="TypeScript" />
</a>
&nbsp;
<a href="#">
  <img src="https://img.shields.io/badge/Solana-compatible-0A1628?style=flat-square&logo=solana&logoColor=D4A843" alt="Solana" />
</a>

*GMGN tells you what the whale did. Stalkr tells you what it will do next.*

</div>

---

## The Problem

Every degen on Solana tracks whale wallets. But every tool tells you what already happened.

| Metric | Reality |
|--------|---------|
| Copy trade loss rate | 60% of copy trades lose money due to delay |
| Average slippage | 15-40% after whale alerts fire |
| Annual opportunity cost | $500M+ from late entries across Solana |

You react. Whales predict. That is the gap.

## The Solution

Stalkr learns whale behavior patterns and predicts their next move before it happens.

### 4-Pattern Analysis Engine

| Pattern | Description |
|---------|-------------|
| **Funding Source** | CEX withdrawal detected. Timer starts. Average delay to first buy calculated from 90-day history. |
| **Time Window** | Active hours identified from UTC histogram. Peak trading windows mapped across 24h cycle. |
| **Sequence** | Token A bought. Conditional probability of Token B computed from historical pair frequency. |
| **Trigger** | Volume spike + price dip conditions evaluated. Buy probability scored against threshold. |

### How It Works

1. **Observe** -- Track whale wallets via Helius Geyser webhooks in real time
2. **Learn** -- Extract 4 behavioral patterns from 90-day transaction history
3. **Predict** -- Cross-match active patterns to generate weighted confidence scores
4. **Alert** -- Send predictions before the whale executes

## Architecture

```mermaid
flowchart TB
    subgraph Data ["Data Ingestion"]
        style Data fill:#0A1628,stroke:#1E6091,color:#D4A843
        H[Helius Geyser] --> C[Collector]
        G[GMGN API] --> C
        B[Birdeye API] --> C
    end

    subgraph Engine ["Pattern Engine"]
        style Engine fill:#152238,stroke:#1E6091,color:#D4A843
        C --> PE[Pattern Extractor]
        PE --> FS[Funding Source]
        PE --> TW[Time Window]
        PE --> SQ[Sequence]
        PE --> TR[Trigger]
    end

    subgraph Predict ["Prediction Layer"]
        style Predict fill:#0A1628,stroke:#D4A843,color:#D4A843
        FS --> SC[Score Calculator]
        TW --> SC
        SQ --> SC
        TR --> SC
        SC --> PF[Prediction Feed]
    end

    subgraph Alert ["Alert System"]
        style Alert fill:#152238,stroke:#D4A843,color:#D4A843
        PF --> TG[Telegram Bot]
        PF --> WP[Web Push]
        PF --> AB[Auto-Buy]
    end

    subgraph Client ["Client"]
        style Client fill:#0A1628,stroke:#1E6091,color:#D4A843
        PF --> WEB[Web Dashboard]
        PF --> SDK[TypeScript SDK]
        PF --> CLI[CLI Tool]
    end

    style H fill:#1E6091,stroke:#D4A843,color:#fff
    style G fill:#1E6091,stroke:#D4A843,color:#fff
    style B fill:#1E6091,stroke:#D4A843,color:#fff
    style C fill:#D4A843,stroke:#0A1628,color:#0A1628
    style PE fill:#D4A843,stroke:#0A1628,color:#0A1628
    style FS fill:#1E6091,stroke:#D4A843,color:#fff
    style TW fill:#1E6091,stroke:#D4A843,color:#fff
    style SQ fill:#1E6091,stroke:#D4A843,color:#fff
    style TR fill:#1E6091,stroke:#D4A843,color:#fff
    style SC fill:#D4A843,stroke:#0A1628,color:#0A1628
    style PF fill:#D4A843,stroke:#0A1628,color:#0A1628
    style TG fill:#1E6091,stroke:#D4A843,color:#fff
    style WP fill:#1E6091,stroke:#D4A843,color:#fff
    style AB fill:#1E6091,stroke:#D4A843,color:#fff
    style WEB fill:#1E6091,stroke:#D4A843,color:#fff
    style SDK fill:#1E6091,stroke:#D4A843,color:#fff
    style CLI fill:#1E6091,stroke:#D4A843,color:#fff
```

## Features

| Feature | Description |
|---------|-------------|
| Prediction Feed | Top 10 whales about to move, ranked by confidence score |
| Pattern Analysis | 4-pattern breakdown per whale with time heatmap, CEX delay, sequence pairs, trigger conditions |
| Whale Tiers | Dolphin ($1M+) / Humpback ($10M+) / Blue Whale ($100M+) classification |
| Telegram Alerts | Real-time notifications for high-confidence predictions (70%+ threshold) |
| Watchlist | Track up to 10 whale wallets with personalized prediction alerts |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Core Engine | Rust + Anchor (on-chain pattern analysis, scoring) |
| Math Library | Rust (statistics, z-scores, histograms, correlation) |
| SDK | TypeScript (client library, type-safe API wrapper) |
| CLI | Rust + Clap (wallet tracking, prediction queries) |
| Frontend | Next.js 15 + Three.js/R3F (ocean 3D scene) |
| Backend | Hono (TypeScript, edge-ready) |
| Data Sources | Helius Geyser + GMGN + Birdeye |
| Database | PostgreSQL + Redis |
| Infrastructure | Vercel + Railway |

## Installation

```bash
git clone https://github.com/stalkr-labs/stalkr.git
cd stalkr

# Build core programs
cargo build --release

# Install SDK dependencies
cd sdk && npm install && cd ..
```

## Usage

```bash
# Track a whale wallet
cargo run --bin stalkr-cli -- track <WALLET_ADDRESS>

# Get prediction feed
cargo run --bin stalkr-cli -- predictions --top 10

# Analyze patterns for a specific wallet
cargo run --bin stalkr-cli -- analyze <WALLET_ADDRESS>

# JSON output
cargo run --bin stalkr-cli -- predictions --top 5 --json
```

### SDK

```typescript
import { StalkrClient } from "@stalkr-labs/stalkr-sdk";

const client = new StalkrClient({ baseUrl: "https://api.stalkr.live" });

const predictions = await client.getTopPredictions(10);
const whale = await client.getWhaleProfile("WALLET_ADDRESS");
const patterns = await client.getPatternAnalysis("WALLET_ADDRESS");
```

## API Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/predictions/top` | GET | Top predictions ranked by confidence |
| `/api/whale/:address` | GET | Whale profile and tier classification |
| `/api/whale/:address/patterns` | GET | 4-pattern breakdown for a wallet |
| `/api/watchlist` | GET | Current watchlist |
| `/api/watchlist` | POST | Add wallet to watchlist |
| `/api/predictions/history` | GET | Historical prediction accuracy |

Full API documentation: [docs/api-reference.md](./docs/api-reference.md)

## Token Utility ($STALKR)

| Mechanism | Detail |
|-----------|--------|
| Buyback & Burn | 50% of prediction fees directed to buyback and burn |
| Tier System | Drifter / Angler / Navigator / Harpooner / Leviathan |
| Access Levels | Higher tiers unlock more predictions, real-time alerts, and auto-buy |

## Documentation

- [Architecture](./docs/architecture.md) -- System design, data flow, infrastructure
- [API Reference](./docs/api-reference.md) -- All endpoints with request/response examples
- [Pattern Analysis](./docs/patterns.md) -- Deep dive into the 4-pattern engine

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

[MIT](LICENSE) -- stalkr-labs

---

<div align="center">

*The ocean remembers every pattern.*

**$STALKR**

</div>
<!-- write architecture overview [1.2] -->
<!-- add trigger condition evaluator [2.2] -->
<!-- implement multi-pattern cross matching [2.17] -->
<!-- add window hours validation 1-168 [2.32] -->
<!-- consolidate api client configuration [3.2] -->
<!-- implement tier display with color coding [4.2] -->
<!-- add distribution helper tests [5.2] -->
