use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Deserialize;
use serde_json::Value;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a session, creating it if it doesn't exist
    Open {
        name: String,
        /// Name of a subfolder inside `.devcontainer` holding `devcontainer.json`
        #[arg(long)]
        devcontainer_env: Option<String>,
    },
    /// Kill a running session
    Kill { name: String },
    /// List running sessions
    Ls,
}

#[derive(Deserialize, Default)]
struct Config {
    githuborg: Option<String>,
}

fn load_config() -> Config {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "forest") {
        let path = proj_dirs.config_dir().join("forest.toml");
        if let Ok(content) = fs::read_to_string(path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Config::default()
        }
    } else {
        Config::default()
    }
}

fn find_devcontainer(dev_env: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(env) = dev_env {
        let candidate = Path::new(".devcontainer").join(env).join("devcontainer.json");
        if candidate.exists() {
            return Ok(candidate);
        }
        anyhow::bail!("devcontainer {} not found", env);
    }

    let root = Path::new(".devcontainer.json");
    if root.exists() {
        return Ok(root.to_path_buf());
    }

    let default = Path::new(".devcontainer").join("devcontainer.json");
    if default.exists() {
        return Ok(default);
    }

    // Scaffold default devcontainer.json
    fs::create_dir_all(".devcontainer")?;
    fs::write(&default, "{\n  \"image\": \"docker.io/library/ubuntu:latest\"\n}\n")?;
    Ok(default)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = load_config();

    match cli.command {
        Commands::Open { name, devcontainer_env } => {
            open_session(&name, devcontainer_env.as_deref(), &config)?
        }
        Commands::Kill { name } => kill_session(&name)?,
        Commands::Ls => list_sessions()?,
    }
    Ok(())
}

fn open_session(name: &str, dev_env: Option<&str>, _config: &Config) -> anyhow::Result<()> {
    let devcontainer_path = find_devcontainer(dev_env)?;

    let contents = fs::read_to_string(&devcontainer_path)?;
    let value: Value = serde_json::from_str(&contents)?;
    let image = value
        .get("image")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("image field missing in devcontainer"))?;

    // Check if container exists
    let exists = Command::new("podman")
        .args(["container", "exists", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if exists {
        println!("Session {} already exists", name);
        return Ok(());
    }

    let status = Command::new("podman")
        .args(["run", "-d", "--name", name, image])
        .status()?;
    if !status.success() {
        anyhow::bail!("podman run failed");
    }
    println!("Started session {}", name);
    Ok(())
}

fn kill_session(name: &str) -> anyhow::Result<()> {
    let status = Command::new("podman").args(["rm", "-f", name]).status()?;
    if !status.success() {
        anyhow::bail!("podman rm failed");
    }
    println!("Killed session {}", name);
    Ok(())
}

fn list_sessions() -> anyhow::Result<()> {
    Command::new("podman").arg("ps").status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::env;

    #[test]
    fn scaffold_created_when_missing() {
        let dir = tempdir().unwrap();
        let orig = env::current_dir().unwrap();
        env::set_current_dir(&dir).unwrap();

        let path = find_devcontainer(None).unwrap();
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("ubuntu"));

        env::set_current_dir(orig).unwrap();
    }
}
