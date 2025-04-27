#!/usr/bin/env -S bash --noprofile --norc
set -euo pipefail

PROJECT_DIR="$HOME/Projects/sw-catcher"

if [[ -d "$PROJECT_DIR" ]]; then
  echo "→ $PROJECT_DIR already exists; skipping creation."
else
  echo "→ Creating new project at $PROJECT_DIR"
  mkdir -p "$PROJECT_DIR"
  cd "$PROJECT_DIR"

  # Initialize a new binary crate
  cargo init --bin .

  # Add our dependencies
  cargo add notify dirs clap toml

  # Write default config.toml
  cat > config.toml << 'EOF'
# default settings for sw-catcher
watch_dir = "$HOME/Documents/superwhisper/recordings"
EOF

  # Overwrite src/main.rs with our watcher skeleton
  cat > src/main.rs << 'EOF'
use clap::Parser;
use dirs::home_dir;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Result, Watcher};
use std::{path::PathBuf, process::Command, thread, time::Duration};

/// Simple watcher that opens any newly-created `meta.json`.
#[derive(Parser)]
#[command(name = "sw-catcher")]
struct Opts {
    /// Directory to watch (override with --watch-dir)
    #[arg(short, long)]
    watch_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    // Load default from config.toml
    let settings: toml::Value = std::fs::read_to_string("config.toml")?
        .parse::<toml::Value>()
        .expect("invalid config.toml");

    let default_dir = settings
        .get("watch_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .expect("watch_dir missing in config.toml");

    // Final watch directory = CLI override or default
    let watch_path = opts.watch_dir.unwrap_or(default_dir);
    println!("Watching: {:?}", watch_path);

    // Build the FS watcher (on macOS uses FSEvents)
    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
        move |res: Result<Event>| match res {
            Ok(e)  => handle_event(e),
            Err(e) => eprintln!("watch error: {:?}", e),
        },
        Config::default(),
    )?;

    watcher.watch(&watch_path, RecursiveMode::Recursive)?;

    // Block forever (Ctrl-C to exit)
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn handle_event(evt: Event) {
    if let EventKind::Create(_) = evt.kind {
        for path in evt.paths {
            if path.file_name().and_then(|s| s.to_str()) == Some("meta.json") {
                println!("Found meta.json: {:?}", path);
                if let Err(e) = Command::new("open").arg(&path).spawn() {
                    eprintln!("Failed to open {}: {}", path.display(), e);
                }
            }
        }
    }
}
EOF

  echo "Project bootstrapped!"
fi

echo "→ cd \$PROJECT_DIR"
echo "→ cargo run  [-- --watch-dir /some/other/path]"