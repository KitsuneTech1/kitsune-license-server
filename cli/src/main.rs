//! Kitsune terminal activator + runtime guard demo.
//!
//!   ktsn-activate            prompt for a key, activate, verify, gate install
//!   ktsn-activate --status   one-shot re-check of the saved license
//!   ktsn-activate --watch    keep checking; print status changes live
//!
//! Config (env): KTSN_SERVER, KTSN_PUBLIC_KEY (or baked-in), KTSN_PRODUCT.
//! Exit 0 = entitled. Non-zero = stop.

use std::io::Write;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use kitsune_license_client::{activate, validate, device_fingerprint, Config, Guard, Status};

mod fox;

const EMBEDDED_PUBLIC_KEY: &str = "Fo20vUMA4HPCdpxPEsccfPBSMExyXVoJhtouJ6bTRnw";
const DEFAULT_SERVER: &str = "https://license.kitsunetechnologies.org";

fn main() {
    let code = match run() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\n  ✖ {e}\n");
            1
        }
    };
    std::process::exit(code);
}

fn config() -> Result<Config> {
    let server = std::env::var("KTSN_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.into());
    let pubkey = std::env::var("KTSN_PUBLIC_KEY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| (!EMBEDDED_PUBLIC_KEY.is_empty()).then(|| EMBEDDED_PUBLIC_KEY.into()))
        .ok_or_else(|| anyhow!("no server public key embedded (set KTSN_PUBLIC_KEY)"))?;
    let mut cfg = Config::new(server, pubkey);
    cfg.product = std::env::var("KTSN_PRODUCT").ok().filter(|s| !s.is_empty());
    Ok(cfg)
}

fn run() -> Result<i32> {
    let cfg = config()?;
    match std::env::args().nth(1).as_deref() {
        Some("--status") => status_once(&cfg),
        Some("--watch") => watch(&cfg),
        _ => do_activate(&cfg),
    }
}

fn do_activate(cfg: &Config) -> Result<i32> {
    let mascot = fox::Mascot::new();
    println!();
    mascot.draw_in();
    println!("\n  Kitsune Technologies — license activation");
    println!("  ─────────────────────────────────────────");
    let key = prompt("  Enter your license key: ")?;
    if key.trim().is_empty() {
        bail!("no key entered");
    }
    print!("  Activating… ");
    std::io::stdout().flush().ok();

    // Run the server call on a worker thread; shimmer the fox while we wait.
    // Lines between the art and the cursor: blank, title, rule, prompt = 4.
    let lines_below: u16 = 4;
    let worker = {
        let cfg = cfg.clone();
        let key = key.clone();
        std::thread::spawn(move || activate(&cfg, &key))
    };
    let mut tick = 0usize;
    while !worker.is_finished() {
        mascot.shimmer_step(tick, lines_below);
        tick += 1;
        std::thread::sleep(Duration::from_millis(70));
    }
    let lic = worker
        .join()
        .map_err(|_| anyhow!("activation thread panicked"))?
        .map_err(|e| anyhow!("{e}"))?;
    mascot.settle(lines_below);
    save_key(cfg, &key)?;
    println!("ok");
    println!("\n  ✓ Activated for '{}'.", lic.product);
    match lic.lic_exp {
        Some(e) => println!("    License valid until unix {e}."),
        None => println!("    License does not expire."),
    }
    println!("    You may continue the installation.\n");
    Ok(0)
}

fn status_once(cfg: &Config) -> Result<i32> {
    let key = load_key(cfg).context("no saved key — run activation first")?;
    let device = device_fingerprint(cfg)?;
    let (status, _) = validate(cfg, &key, &device).map_err(|e| anyhow!("{e}"))?;
    println!("license status: {status:?}");
    Ok(if status.is_entitled() { 0 } else { 1 })
}

fn watch(cfg: &Config) -> Result<i32> {
    let key = load_key(cfg).context("no saved key — run activation first")?;
    let interval = Duration::from_secs(
        std::env::var("KTSN_INTERVAL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30),
    );
    println!("  watching license (every {}s) — Ctrl-C to stop\n", interval.as_secs());
    let guard = Guard::start(
        cfg.clone(),
        key,
        interval,
        Duration::from_secs(7 * 24 * 60 * 60), // a week of offline grace
        |status: Status| {
            let mark = if status.is_entitled() { "✓" } else { "✖" };
            println!("  {mark} status: {status:?}");
        },
    );
    // Park forever; the Guard runs in its own thread.
    loop {
        std::thread::sleep(Duration::from_secs(3600));
        let _ = &guard;
    }
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    std::io::stdout().flush().ok();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

// The key is the user's own secret on their own machine; cache it next to the
// license so --status/--watch can re-validate without re-prompting.
fn save_key(cfg: &Config, key: &str) -> Result<()> {
    std::fs::create_dir_all(&cfg.config_dir)?;
    std::fs::write(cfg.config_dir.join("key"), key.trim())?;
    Ok(())
}

fn load_key(cfg: &Config) -> Option<String> {
    std::fs::read_to_string(cfg.config_dir.join("key"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
