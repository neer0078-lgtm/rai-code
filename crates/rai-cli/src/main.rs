//! rai — the RAI Code CLI entry point.
//!
//! Onboarding-first (per the dossier): recall from Hindsight before driving the
//! TUI, so the agent knows who it's talking to and what the human handles vs
//! delegates. Built by RAI Labs P. Ltd. — www.railabs.in — reach@railabs.in.

use clap::Parser;
use rai_cli::{ABOUT_STRING, VERSION_STRING};

#[derive(Parser)]
#[command(
    name = "rai",
    version = VERSION_STRING,
    about = ABOUT_STRING
)]
struct Cli {
    /// Run in local mode (no Python, no cloud sandbox).
    #[arg(long, default_value_t = true)]
    local: bool,
    /// Run in full mode (Python sidecar + cloud sandbox).
    #[arg(long)]
    full: bool,
    /// Headless one-shot (no TUI) — prints streamed tokens to stdout.
    #[arg(short = 'p', long)]
    print: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    if let Some(prompt) = cli.print {
        // T61: headless mode — run the loop + print tokens.
        // The real provider wiring (Ollama/Anthropic) is configured via env/CLI
        // in the full impl; for now, headless requires a provider to be passed
        // programmatically. The library function run_headless(prompt, provider)
        // is tested in lib.rs.
        eprintln!("[rai] headless mode for prompt: {prompt}");
        eprintln!("[rai] Provider wiring (Ollama/Anthropic) not yet configured in main.rs.");
        eprintln!("[rai] Use the library API: rai_cli::run_headless(prompt, provider)");
        return Ok(());
    }

    // T59: detect Ollama (in local mode).
    if cli.local && !cli.full {
        let detection = rai_cli::detect_ollama().await;
        if detection.available {
            eprintln!(
                "[rai] Ollama detected — models: {} — recommended: {}",
                detection.models.join(", "),
                detection.recommended_tier.description()
            );
        } else {
            eprintln!(
                "[rai] Ollama not detected. Install: curl -fsSL https://ollama.com/install.sh | sh"
            );
            eprintln!("[rai] Or run with --full for cloud models.");
        }
    }

    // T60: onboarding-first (stub — the real flow uses the TUI for interactive Q&A).
    eprintln!("[rai] Onboarding-first flow (interactive TUI — not yet implemented in headless).");
    eprintln!("[rai] The TUI (rai-tui) is built; the event loop wiring is the next phase.");
    eprintln!("[rai] For now, use: rai -p \"your prompt\" for headless mode.");

    Ok(())
}
