use clap::{Parser, Subcommand};
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "stalkr-cli")]
#[command(about = "Predictive wallet intelligence platform for Solana")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// API base URL
    #[arg(long, default_value = "https://api.stalkr.live")]
    api_url: String,

    /// Output as JSON
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Track a whale wallet
    Track {
        /// Wallet address to track
        address: String,
    },
    /// Get top predictions
    Predictions {
        /// Number of results
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
    /// Analyze patterns for a wallet
    Analyze {
        /// Wallet address to analyze
        address: String,
    },
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct PredictionItem {
    whale_address: String,
    confidence: u8,
    predicted_action: String,
    window_hours: u16,
    tier: String,
}

#[derive(Deserialize)]
struct PatternResult {
    funding_source: FundingData,
    time_window: TimeData,
    sequence: SequenceData,
    trigger: TriggerData,
}

#[derive(Deserialize)]
struct FundingData {
    avg_delay_minutes: f64,
    primary_cex: String,
    sample_count: u32,
}

#[derive(Deserialize)]
struct TimeData {
    peak_hour: u8,
    is_currently_active: bool,
}

#[derive(Deserialize)]
struct SequenceData {
    pairs: Vec<TokenPair>,
}

#[derive(Deserialize)]
struct TokenPair {
    token_a: String,
    token_b: String,
    probability: f64,
}

#[derive(Deserialize)]
struct TriggerData {
    active_triggers_count: u32,
}

fn tier_label(tier: &str) -> &str {
    match tier {
        "dolphin" => "Dolphin ($1M+)",
        "humpback" => "Humpback ($10M+)",
        "blue" => "Blue Whale ($100M+)",
        _ => "Unknown",
    }
}

fn format_confidence(confidence: u8) -> String {
    let bar_len = (confidence as usize).saturating_sub(50) / 5;
    let bar: String = (0..bar_len).map(|_| '#').collect();
    let empty: String = (0..(10 - bar_len)).map(|_| '-').collect();
    format!("[{}{}] {}%", bar, empty, confidence)
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Track { address } => {
            println!("Tracking wallet: {}", address);
            println!("Fetching profile from {}/api/whale/{}", cli.api_url, address);

            let url = format!("{}/api/whale/{}", cli.api_url, address);
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if cli.json {
                        println!("{}", resp.text().unwrap_or_default());
                    } else {
                        match resp.json::<ApiResponse<serde_json::Value>>() {
                            Ok(data) => {
                                if data.success {
                                    println!("Wallet tracked successfully");
                                    if let Some(val) = data.data {
                                        println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
                                    }
                                } else {
                                    eprintln!("Error: {}", data.error.unwrap_or_default());
                                }
                            }
                            Err(e) => eprintln!("Failed to parse response: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Request failed: {}", e),
            }
        }
        Commands::Predictions { top } => {
            println!("Fetching top {} predictions...", top);
            let url = format!("{}/api/predictions/top?limit={}", cli.api_url, top);
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if cli.json {
                        println!("{}", resp.text().unwrap_or_default());
                    } else {
                        match resp.json::<ApiResponse<Vec<PredictionItem>>>() {
                            Ok(data) => {
                                if let Some(predictions) = data.data {
                                    println!("\n--- Prediction Feed ---\n");
                                    for (i, p) in predictions.iter().enumerate() {
                                        println!(
                                            "#{} | {} | {} | {} | {}h window",
                                            i + 1,
                                            &p.whale_address[..8],
                                            tier_label(&p.tier),
                                            format_confidence(p.confidence),
                                            p.window_hours
                                        );
                                    }
                                }
                            }
                            Err(e) => eprintln!("Failed to parse response: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Request failed: {}", e),
            }
        }
        Commands::Analyze { address } => {
            println!("Analyzing patterns for: {}", address);
            let url = format!("{}/api/whale/{}/patterns", cli.api_url, address);
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if cli.json {
                        println!("{}", resp.text().unwrap_or_default());
                    } else {
                        match resp.json::<ApiResponse<PatternResult>>() {
                            Ok(data) => {
                                if let Some(patterns) = data.data {
                                    println!("\n--- Pattern Analysis ---\n");
                                    println!(
                                        "Funding Source: avg delay {:.0}min | CEX: {} | samples: {}",
                                        patterns.funding_source.avg_delay_minutes,
                                        patterns.funding_source.primary_cex,
                                        patterns.funding_source.sample_count
                                    );
                                    println!(
                                        "Time Window: peak hour {}:00 UTC | active now: {}",
                                        patterns.time_window.peak_hour,
                                        patterns.time_window.is_currently_active
                                    );
                                    println!("Sequence Pairs:");
                                    for pair in &patterns.sequence.pairs {
                                        println!(
                                            "  {} -> {} ({:.0}%)",
                                            pair.token_a, pair.token_b, pair.probability * 100.0
                                        );
                                    }
                                    println!(
                                        "Active Triggers: {}",
                                        patterns.trigger.active_triggers_count
                                    );
                                }
                            }
                            Err(e) => eprintln!("Failed to parse response: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Request failed: {}", e),
            }
        }
    }
}

// correct argument parsing for wallet addresses

// add distribution helper tests

// handle wallet address validation

// add verbose output flag

// handle empty prediction response

// add export to csv functionality

// add shell completion generation

// add watch mode for continuous updates (#56)

// add pattern summary in analyze output

// improve error message formatting

// improve subcommand help text

// correct output alignment in table mode
