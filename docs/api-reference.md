# API Reference

Base URL: `https://api.stalkr.live`

All responses follow the standard envelope format:

```json
{
  "success": true,
  "data": { },
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

## Endpoints

### GET /health

Health check endpoint.

**Response:**

```json
{
  "success": true,
  "data": {
    "status": "ok",
    "version": "0.4.2"
  },
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

---

### GET /api/predictions/top

Returns the top whales predicted to move next, ranked by confidence score.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| limit | number | 10 | Number of results (max 20) |

**Response:**

```json
{
  "success": true,
  "data": [
    {
      "id": "pred_abc123",
      "whaleAddress": "7xKX...",
      "tier": "humpback",
      "confidence": 87,
      "predictedAction": "buy_memecoin",
      "predictedTokenType": "memecoin",
      "predictedMcapRange": "1M-10M",
      "windowHours": 6,
      "patternsMatched": {
        "fundingSource": true,
        "timeWindow": true,
        "sequence": true,
        "trigger": false
      },
      "createdAt": "2026-03-10T11:30:00.000Z"
    }
  ],
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

---

### GET /api/whale/:address

Returns the full profile for a tracked whale wallet.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| address | string | Solana wallet address |

**Response:**

```json
{
  "success": true,
  "data": {
    "address": "7xKX...",
    "tier": "humpback",
    "totalValueUsd": 45200000,
    "overallAccuracy": 0.72,
    "totalPredictions": 48,
    "correctPredictions": 35,
    "lastActive": "2026-03-10T09:15:00.000Z",
    "label": "Smart Money #7"
  },
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

---

### GET /api/whale/:address/patterns

Returns the 4-pattern analysis results for a specific whale.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| address | string | Solana wallet address |

**Response:**

```json
{
  "success": true,
  "data": {
    "fundingSource": {
      "avgDelayMinutes": 47,
      "medianDelayMinutes": 42,
      "stdDevMinutes": 15,
      "sampleCount": 23,
      "lastCexWithdrawal": "2026-03-10T08:00:00.000Z",
      "primaryCex": "Binance"
    },
    "timeWindow": {
      "activeHoursUtc": [14, 15, 16],
      "histogram": {"0": 2, "1": 1, "14": 28, "15": 31, "16": 25},
      "isCurrentlyActive": true,
      "peakHour": 15
    },
    "sequence": {
      "pairs": [
        {
          "tokenA": "SOL",
          "tokenB": "WIF",
          "probability": 0.78,
          "avgWindowHours": 24
        }
      ],
      "lastBoughtToken": "SOL"
    },
    "trigger": {
      "conditions": [
        {
          "metric": "volume_change",
          "threshold": 5,
          "buyProbability": 0.82
        }
      ],
      "activeTriggersCount": 1
    }
  },
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

---

### POST /api/watchlist

Add a wallet to the authenticated user's watchlist.

**Request Body:**

```json
{
  "address": "7xKX..."
}
```

**Response:**

```json
{
  "success": true,
  "data": {
    "address": "7xKX...",
    "addedAt": "2026-03-10T12:00:00.000Z",
    "label": null,
    "tier": "humpback",
    "lastPrediction": null
  },
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

---

### GET /api/watchlist

Returns the authenticated user's watchlist.

**Response:**

```json
{
  "success": true,
  "data": [
    {
      "address": "7xKX...",
      "addedAt": "2026-03-10T10:00:00.000Z",
      "label": "Big Player",
      "tier": "blue",
      "lastPrediction": {
        "confidence": 91,
        "predictedAction": "buy_memecoin",
        "windowHours": 4
      }
    }
  ],
  "error": null,
  "timestamp": "2026-03-10T12:00:00.000Z"
}
```

---

### POST /api/webhook/helius

Receives real-time transaction webhooks from Helius Geyser. This endpoint is called by Helius and requires webhook secret validation.

**Headers:**

| Header | Description |
|--------|-------------|
| x-webhook-secret | Helius webhook verification secret |

---

### GET /api/predictions/history

Returns historical prediction records with resolution status.

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| limit | number | 20 | Number of results (max 100) |
| offset | number | 0 | Pagination offset |

---

### POST /api/auto-buy/setup

Configure automatic pre-emptive buying based on prediction signals.

**Request Body:**

```json
{
  "whaleAddress": "7xKX...",
  "maxAmountSol": 1.0,
  "minConfidence": 80,
  "slippageBps": 300
}
```

<!-- updated -->

<!-- updated -->
