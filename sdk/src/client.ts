import type {
  WhaleProfile,
  Prediction,
  PatternAnalysis,
  WatchlistItem,
  ApiResponse,
} from "./types";

interface ClientConfig {
  baseUrl: string;
  timeout?: number;
}

export class StalkrClient {
  private baseUrl: string;
  private timeout: number;

  constructor(config: ClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, "");
    this.timeout = config.timeout ?? 10000;
  }

  private async request<T>(path: string): Promise<T> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    const response = await fetch(`${this.baseUrl}${path}`, {
      signal: controller.signal,
      headers: { "Content-Type": "application/json" },
    });

    clearTimeout(timer);

    if (!response.ok) {
      throw new Error(`Request failed with status ${response.status}`);
    }

    const body: ApiResponse<T> = await response.json();

    if (!body.success || body.data === null) {
      throw new Error(body.error ?? "Unknown error");
    }

    return body.data;
  }

  async getTopPredictions(limit: number = 10): Promise<Prediction[]> {
    return this.request<Prediction[]>(`/api/predictions/top?limit=${limit}`);
  }

  async getWhaleProfile(address: string): Promise<WhaleProfile> {
    return this.request<WhaleProfile>(`/api/whale/${address}`);
  }

  async getPatternAnalysis(address: string): Promise<PatternAnalysis> {
    return this.request<PatternAnalysis>(`/api/whale/${address}/patterns`);
  }

  async getWatchlist(): Promise<WatchlistItem[]> {
    return this.request<WatchlistItem[]>("/api/watchlist");
  }

  async addToWatchlist(address: string): Promise<WatchlistItem> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    const response = await fetch(`${this.baseUrl}/api/watchlist`, {
      method: "POST",
      signal: controller.signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ address }),
    });

    clearTimeout(timer);

    if (!response.ok) {
      throw new Error(`Request failed with status ${response.status}`);
    }

    const body: ApiResponse<WatchlistItem> = await response.json();

    if (!body.success || body.data === null) {
      throw new Error(body.error ?? "Unknown error");
    }

    return body.data;
  }

  async getPredictionHistory(
    limit: number = 20,
    offset: number = 0
  ): Promise<Prediction[]> {
    return this.request<Prediction[]>(
      `/api/predictions/history?limit=${limit}&offset=${offset}`
    );
  }
}

// add webhook event type definitions

// reduce allocation count in hot path

// resolve remaining compiler warnings

// correct exit code on prediction fetch failure

// add whale tier filter to predictions

// add retry logic for failed requests

// add pagination support for history

// handle network timeout in batch requests
