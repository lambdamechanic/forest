use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Deserialize;
use serde_json::Value;

use std::process::Stdio;

fn ensure_git_setup(branch: &str, config: &Config, verbose: bool) -> anyhow::Result<()> {
    // Are we inside a git repository?
    if verbose {
        println!("Checking git repository root");
    }
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stderr(Stdio::null())
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
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !branch_exists {
        if verbose {
            println!("Creating git branch {}", branch);
        }
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
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !remote_exists {
        if verbose {
            println!("Creating origin remote");
        }
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
    /// Print debugging information
    #[arg(short, long)]
    verbose: bool,
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
    /// Verify prerequisites are installed and config is valid
    Precheck,
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

    let verbose = cli.verbose;

    match cli.command {
        Commands::Open {
            name,
            devcontainer_env,
        } => open_session(&name, devcontainer_env.as_deref(), &config, verbose)?,
        Commands::Kill { name } => kill_session(&name, verbose)?,
        Commands::Ls => list_sessions(verbose)?,
        Commands::Precheck => precheck(verbose)?,
    }
    Ok(())
}

fn open_session(
    name: &str,
    dev_env: Option<&str>,
    config: &Config,
    verbose: bool,
) -> anyhow::Result<()> {
    ensure_git_setup(name, config, verbose)?;

    // Determine repository root and worktree path
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stderr(Stdio::null())
        .output()?;
    let repo_root = PathBuf::from(str::from_utf8(&output.stdout)?.trim());
    let repo_name = repo_root
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("failed to determine repo name"))?
        .to_string_lossy();

    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let worktree_root = Path::new(&home).join("worktrees").join(&*repo_name);
    let worktree_path = worktree_root.join(name);

    if !worktree_path.exists() {
        if verbose {
            println!("Creating worktree at {}", worktree_path.display());
        }
        fs::create_dir_all(&worktree_root)?;
        let status = Command::new("git")
            .args([
                "worktree",
                "add",
                "-B",
                name,
                worktree_path.to_str().unwrap(),
            ])
            .current_dir(&repo_root)
            .status()?;
        if !status.success() {
            anyhow::bail!("git worktree add failed");
        }
    }
    let devcontainer_path = find_devcontainer(dev_env)?;

    if verbose {
        println!("Using devcontainer at {}", devcontainer_path.display());
    }

    let contents = fs::read_to_string(&devcontainer_path)?;
    let value: Value = serde_json::from_str(&contents)?;
    let image = if let Some(img) = value.get("image").and_then(|v| v.as_str()) {
        img.to_string()
    } else if let Some(build) = value.get("build") {
        let dockerfile = build
            .get("dockerfile")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("image field missing in devcontainer"))?;
        let context = build.get("context").and_then(|v| v.as_str()).unwrap_or(".");
        let dockerfile_path = devcontainer_path.parent().unwrap().join(dockerfile);
        let context_path = devcontainer_path.parent().unwrap().join(context);
        let tag = format!("{}-image", name);
        if verbose {
            println!(
                "Running: podman build -t {} -f {} {}",
                tag,
                dockerfile_path.display(),
                context_path.display()
            );
        }
        let status = Command::new("podman")
            .arg("build")
            .arg("-t")
            .arg(&tag)
            .arg("-f")
            .arg(&dockerfile_path)
            .arg(&context_path)
            .status()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("podman command not found. Please install podman")
                } else {
                    e.into()
                }
            })?;
        if !status.success() {
            anyhow::bail!("podman build failed");
        }
        tag
    } else {
        anyhow::bail!("image field missing in devcontainer");
    };

    // Check if container exists
    if verbose {
        println!("Checking if container {} exists", name);
    }
    let exists = Command::new("podman")
        .args(["container", "exists", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if exists {
        println!("Session {} already exists", name);
    } else {
        if verbose {
            println!(
                "Running: podman run -d --name {} -v {}:/repo -v {}:{} -v {}:/code -w /code --init {} sleep infinity",
                name,
                repo_root.display(),
                repo_root.display(),
                repo_root.display(),
                worktree_path.display(),
                image
            );
        }
        let status = Command::new("podman")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(name)
            .arg("-v")
            .arg(format!("{}:/repo", repo_root.display()))
            .arg("-v")
            .arg(format!("{}:{}", repo_root.display(), repo_root.display()))
            .arg("-v")
            .arg(format!("{}:/code", worktree_path.display()))
            .arg("-w")
            .arg("/code")
            .arg("--init")
            .arg(image)
            .arg("sleep")
            .arg("infinity")
            .status()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("podman command not found. Please install podman")
                } else {
                    e.into()
                }
            })?;
        if !status.success() {
            anyhow::bail!("podman run failed");
        }
        println!("Started session {}", name);
    }
    if verbose {
        println!("Running: podman exec -it {} bash", name);
    }
    let status = Command::new("podman")
        .args(["exec", "-it", name, "bash"])
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("podman command not found. Please install podman")
            } else {
                e.into()
            }
        })?;
    if !status.success() {
        anyhow::bail!("podman exec failed");
    }
    Ok(())
}

fn kill_session(name: &str, verbose: bool) -> anyhow::Result<()> {
    if verbose {
        println!("Running: podman rm -f {}", name);
    }
    let status = Command::new("podman")
        .args(["rm", "-f", name])
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("podman command not found. Please install podman")
            } else {
                e.into()
            }
        })?;
    if !status.success() {
        anyhow::bail!("podman rm failed");
    }
    println!("Killed session {}", name);
    Ok(())
}

fn list_sessions(verbose: bool) -> anyhow::Result<()> {
    if verbose {
        println!("Running: podman ps");
    }
    Command::new("podman").arg("ps").status().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("podman command not found. Please install podman")
        } else {
            e.into()
        }
    })?;
    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn precheck(verbose: bool) -> anyhow::Result<()> {
    let mut errors = Vec::new();

    for cmd in ["podman", "git", "gh"] {
        if verbose {
            println!("Checking for {}", cmd);
        }
        if !command_exists(cmd) {
            errors.push(format!("{} command not found", cmd));
        }
    }

    if let Some(proj_dirs) = ProjectDirs::from("", "", "forest") {
        let path = proj_dirs.config_dir().join("forest.toml");
        if verbose {
            println!("Checking config {}", path.display());
        }
        match fs::read_to_string(&path) {
            Ok(content) => {
                if let Err(e) = toml::from_str::<Config>(&content) {
                    errors.push(format!("failed to parse {}: {}", path.display(), e));
                }
            }
            Err(_) => errors.push(format!("config file {} not found", path.display())),
        }
    } else {
        errors.push("could not determine configuration directory".to_string());
    }

    if errors.is_empty() {
        if verbose {
            println!("All checks passed");
        }
        Ok(())
    } else {
        println!("Precheck found issues:");
        for e in &errors {
            println!("- {}", e);
        }
        let joined = errors
            .iter()
            .map(|e| format!("- {}", e))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!("precheck failed:\n{}", joined)
    }
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

    #[test]
    fn command_exists_detects_commands() {
        assert!(command_exists("true"));
        assert!(!command_exists("definitely_not_a_command"));
    }

    #[test]
    fn precheck_collects_multiple_errors() {
        let bin_dir = tempdir().unwrap();
        let git_path = bin_dir.path().join("git");
        fs::write(&git_path, "#!/bin/sh\nexit 0\n").unwrap();
        Command::new("/usr/bin/chmod")
            .arg("+x")
            .arg(&git_path)
            .status()
            .unwrap();

        env::set_var("PATH", bin_dir.path());

        let home_dir = tempdir().unwrap();
        env::set_var("HOME", home_dir.path());
        env::set_var("XDG_CONFIG_HOME", home_dir.path());

        let result = precheck(false);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("podman command not found"));
        assert!(err.contains("gh command not found"));
        assert!(err.contains("config file"));
    }

    #[test]
    fn precheck_succeeds_with_all_requirements() {
        let bin_dir = tempdir().unwrap();
        for cmd in ["git", "podman", "gh"] {
            let path = bin_dir.path().join(cmd);
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            Command::new("/usr/bin/chmod")
                .arg("+x")
                .arg(&path)
                .status()
                .unwrap();
        }
        env::set_var("PATH", bin_dir.path());

        let home_dir = tempdir().unwrap();
        env::set_var("HOME", home_dir.path());
        env::set_var("XDG_CONFIG_HOME", home_dir.path());
        let config_dir = home_dir.path().join("forest");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("forest.toml"), "githuborg = 'foo'\n").unwrap();

        assert!(precheck(false).is_ok());
    }
}
