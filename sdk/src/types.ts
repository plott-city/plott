export type WhaleTier = "dolphin" | "humpback" | "blue";

export type TransactionType =
  | "swap"
  | "transfer"
  | "cex_withdrawal"
  | "cex_deposit";

export type PredictedAction =
  | "buy_memecoin"
  | "buy_defi"
  | "buy_ai_token"
  | "sell"
  | "transfer_cex";

export interface FundingSourcePattern {
  avgDelayMinutes: number;
  medianDelayMinutes: number;
  stdDevMinutes: number;
  sampleCount: number;
  lastCexWithdrawal: string | null;
  primaryCex: string;
}

export interface TimeWindowPattern {
  activeHoursUtc: number[];
  histogram: Record<number, number>;
  isCurrentlyActive: boolean;
  peakHour: number;
}

export interface SequencePattern {
  pairs: Array<{
    tokenA: string;
    tokenB: string;
    probability: number;
    avgWindowHours: number;
  }>;
  lastBoughtToken: string | null;
}

export interface TriggerPattern {
  conditions: Array<{
    metric: "volume_change" | "price_change";
    threshold: number;
    buyProbability: number;
  }>;
  activeTriggersCount: number;
}

export interface WhaleProfile {
  address: string;
  tier: WhaleTier;
  totalValueUsd: number;
  fundingSource: FundingSourcePattern;
  timeWindow: TimeWindowPattern;
  sequence: SequencePattern;
  trigger: TriggerPattern;
  overallAccuracy: number;
  totalPredictions: number;
  correctPredictions: number;
  lastActive: string;
  label: string | null;
}

export interface Prediction {
  id: string;
  whaleAddress: string;
  tier: WhaleTier;
  confidence: number;
  predictedAction: PredictedAction;
  predictedTokenType: string;
  predictedMcapRange: string;
  windowHours: number;
  patternsMatched: {
    fundingSource: boolean;
    timeWindow: boolean;
    sequence: boolean;
    trigger: boolean;
  };
  createdAt: string;
  resolvedAt?: string;
  wasCorrect?: boolean;
}

export interface PatternAnalysis {
  fundingSource: FundingSourcePattern;
  timeWindow: TimeWindowPattern;
  sequence: SequencePattern;
  trigger: TriggerPattern;
}

export interface WatchlistItem {
  address: string;
  addedAt: string;
  label: string | null;
  tier: WhaleTier;
  lastPrediction: Prediction | null;
}

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
  timestamp: string;
}

// add whale profile type with all pattern fields [3.17]

// add sequence pair display in analyze output [4.17]

// bump stalkr-math dependency path [7.17]
