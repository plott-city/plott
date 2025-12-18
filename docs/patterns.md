# Pattern Analysis System

Stalkr analyzes whale wallets using four distinct behavioral patterns. Each pattern is extracted from 90-day transaction history and produces a normalized signal used in prediction scoring.

## 1. Funding Source Pattern

Tracks the relationship between CEX withdrawals and subsequent on-chain purchases.

### How It Works

1. Identify transactions from known CEX hot wallet addresses (Binance, OKX, Bybit, Coinbase, Kraken)
2. Measure the delay between each CEX withdrawal and the first subsequent token purchase
3. Build a statistical distribution: mean, median, and standard deviation of delays
4. When a new CEX withdrawal is detected, the timer starts and the predicted buy window is calculated

### Signal

The pattern fires when a CEX withdrawal is detected and the current elapsed time falls within the historical delay distribution (within 1 standard deviation of the mean).

### Scoring

| Metric | Weight |
|--------|--------|
| Recency of last withdrawal | 40% |
| Match with historical delay | 35% |
| Sample count reliability | 25% |

## 2. Time Window Pattern

Maps active trading hours to identify when a whale is most likely to execute trades.

### How It Works

1. Build a 24-hour UTC histogram from all transactions in the 90-day window
2. Identify the top 3 peak activity hours
3. Track whether the current time falls within the active window
4. Weight recent transactions more heavily using exponential decay

### Signal

The pattern fires when the current UTC hour matches one of the whale's top 3 active hours.

### Scoring

| Metric | Weight |
|--------|--------|
| Peak hour match | 45% |
| Transaction density at current hour | 30% |
| Recency of activity in this window | 25% |

## 3. Sequence Pattern

Detects token purchase sequences where buying Token A historically precedes buying Token B.

### How It Works

1. Scan all buy transactions and identify sequential pairs within a 72-hour window
2. Calculate the conditional probability P(Token B | Token A) for each pair
3. Track the average time gap between the paired purchases
4. When a new purchase matches Token A in a known pair, the sequence signal activates

### Signal

The pattern fires when the whale's most recent purchase matches the first token in a high-probability sequence pair.

### Scoring

| Metric | Weight |
|--------|--------|
| Conditional probability of pair | 50% |
| Recency of Token A purchase | 30% |
| Number of historical occurrences | 20% |

## 4. Trigger Pattern

Identifies market conditions (price dips, volume spikes) that historically precede whale buys.

### How It Works

1. For each historical buy, record the market conditions at the time of purchase (token price change, volume change)
2. Cluster conditions into trigger profiles (e.g., "buys when volume spikes 5x and price drops 15%")
3. Monitor current market conditions and match against trigger profiles
4. Calculate the buy probability for each active trigger condition

### Signal

The pattern fires when current market conditions match at least one trigger profile with buy probability above 55%.

### Scoring

| Metric | Weight |
|--------|--------|
| Buy probability of matched trigger | 45% |
| Number of active triggers | 30% |
| Historical accuracy of trigger | 25% |

## Combined Scoring

The four pattern signals are combined using a weighted average:

| Pattern | Weight |
|---------|--------|
| Funding Source | 0.30 |
| Time Window | 0.25 |
| Sequence | 0.25 |
| Trigger | 0.20 |

The final confidence score is normalized to a 50-95% range. Predictions with scores above the threshold (default 70%) trigger alerts. The prediction time window is inversely correlated with confidence: higher confidence produces a shorter, more precise window.

### Threshold Rules

| Matched Patterns | Typical Confidence | Alert Level |
|------------------|--------------------|-------------|
| 4 of 4 | 85-95% | High priority |
| 3 of 4 | 70-84% | Standard alert |
| 2 of 4 | 55-69% | Low priority (watchlist only) |
| 1 of 4 | 50-54% | No alert |
