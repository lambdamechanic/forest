use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Deserialize;
use serde_json::Value;

fn ensure_git_setup(branch: &str, config: &Config) -> anyhow::Result<()> {
    // Are we inside a git repository?
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();
    let repo_root = match output {
        Ok(o) if o.status.success() => {
            let path = str::from_utf8(&o.stdout)?.trim();
            PathBuf::from(path)
        }
        _ => return Ok(()),
    };

    // Check if branch exists
    let branch_exists = Command::new("git")
        .args(["show-ref", "--verify", &format!("refs/heads/{}", branch)])
        .current_dir(&repo_root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !branch_exists {
        let status = Command::new("git")
            .args(["branch", branch])
            .current_dir(&repo_root)
            .status()?;
        if !status.success() {
            anyhow::bail!("git branch failed");
        }
    }

    // Check remote 'origin'
    let remote_exists = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&repo_root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !remote_exists {
        if let Some(org) = &config.githuborg {
            let repo_name = repo_root.file_name().unwrap_or_default().to_string_lossy();
            let repo_spec = format!("{}/{}", org, repo_name);
            let status = Command::new("gh")
                .args([
                    "repo",
                    "create",
                    &repo_spec,
                    "--source",
                    repo_root.to_str().unwrap(),
                    "--remote",
                    "origin",
                    "--push",
                ])
                .status()?;
            if !status.success() {
                anyhow::bail!("gh repo create failed");
            }
        }
    }
    Ok(())
}

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
        let candidate = Path::new(".devcontainer")
            .join(env)
            .join("devcontainer.json");
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
    fs::write(
        &default,
        "{\n  \"image\": \"docker.io/library/ubuntu:latest\"\n}\n",
    )?;
    Ok(default)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = load_config();

    match cli.command {
        Commands::Open {
            name,
            devcontainer_env,
        } => open_session(&name, devcontainer_env.as_deref(), &config)?,
        Commands::Kill { name } => kill_session(&name)?,
        Commands::Ls => list_sessions()?,
    }
    Ok(())
}

fn open_session(name: &str, dev_env: Option<&str>, config: &Config) -> anyhow::Result<()> {
    ensure_git_setup(name, config)?;
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
    use std::env;
    use tempfile::tempdir;

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
