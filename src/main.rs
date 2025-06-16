use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::Deserialize;
use serde_json::Value;

use std::process::Stdio;

fn run_command_verbose(
    cmd: &mut Command,
    verbose: bool,
) -> std::io::Result<std::process::ExitStatus> {
    if verbose {
        println!("Running: {:?}", cmd);
    }
    cmd.status()
}

fn sanitize_podman_name(branch: &str) -> String {
    let mut name: String = branch
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if name
        .chars()
        .next()
        .map(|c| !c.is_ascii_alphanumeric())
        .unwrap_or(true)
    {
        name.insert(0, 's');
    }
    name
}

fn valid_podman_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

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
        let mut cmd = Command::new("git");
        cmd.args(["branch", branch]).current_dir(&repo_root);
        let status = run_command_verbose(&mut cmd, verbose)?;
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
            let mut cmd = Command::new("gh");
            cmd.args([
                "repo",
                "create",
                &repo_spec,
                "--source",
                repo_root.to_str().unwrap(),
                "--remote",
                "origin",
                "--push",
            ]);
            let status = run_command_verbose(&mut cmd, verbose)?;
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

    let podman_name = sanitize_podman_name(name);
    if !valid_podman_name(&podman_name) {
        anyhow::bail!("invalid session name: {}", name);
    }

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
            println!("Creating worktree directory {}", worktree_path.display());
        }
        fs::create_dir_all(&worktree_path)?;
    }
    let devcontainer_path = find_devcontainer(dev_env)?;

    if verbose {
        println!("Using devcontainer at {}", devcontainer_path.display());
    }

    let contents = fs::read_to_string(&devcontainer_path)?;
    let value: Value = serde_json::from_str(&contents)?;
    if value.get("image").is_none() && value.get("build").is_none() {
        anyhow::bail!("image field missing in devcontainer");
    }

    if value.get("build").is_some() {
        let mut cmd = Command::new("devcontainer");
        cmd.arg("build")
            .arg("--workspace-folder")
            .arg(&worktree_path);
        let status = run_command_verbose(&mut cmd, verbose).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("devcontainer command not found. Please install @devcontainers/cli")
            } else {
                e.into()
            }
        })?;
        if !status.success() {
            anyhow::bail!("devcontainer build failed");
        }
    }

    let mut cmd = Command::new("devcontainer");
    cmd.arg("up")
        .arg("--workspace-folder")
        .arg(&worktree_path)
        .arg("--id-label")
        .arg(format!("name={}", podman_name))
        .arg("--mount")
        .arg(format!(
            "type=bind,source={},target=/repo",
            repo_root.display()
        ))
        .arg("--mount")
        .arg(format!(
            "type=bind,source={},target=/code",
            worktree_path.display()
        ));
    let status = run_command_verbose(&mut cmd, verbose).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("devcontainer command not found. Please install @devcontainers/cli")
        } else {
            e.into()
        }
    })?;
    if !status.success() {
        anyhow::bail!("devcontainer up failed");
    }
    println!("Started session {}", name);

    let git_file = worktree_path.join(".git");
    let mut need_worktree = true;
    if let Ok(content) = fs::read_to_string(&git_file) {
        if content.contains("/repo/.git/worktrees/") {
            need_worktree = false;
        }
    }
    if need_worktree {
        let mut cmd = Command::new("devcontainer");
        cmd.arg("exec")
            .arg("--workspace-folder")
            .arg(&worktree_path)
            .arg("--id-label")
            .arg(format!("name={}", podman_name))
            .arg("bash")
            .arg("-lc")
            .arg(format!("git -C /repo worktree add -B {} /code", name));
        let status = run_command_verbose(&mut cmd, verbose).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("devcontainer command not found. Please install @devcontainers/cli")
            } else {
                e.into()
            }
        })?;
        if !status.success() {
            anyhow::bail!("git worktree add failed");
        }
    }

    let mut cmd = Command::new("devcontainer");
    cmd.arg("exec")
        .arg("--workspace-folder")
        .arg(&worktree_path)
        .arg("--id-label")
        .arg(format!("name={}", podman_name))
        .arg("bash")
        .arg("-lc")
        .arg("cd /code && exec bash");
    let status = run_command_verbose(&mut cmd, verbose).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("devcontainer command not found. Please install @devcontainers/cli")
        } else {
            e.into()
        }
    })?;
    if !status.success() {
        anyhow::bail!("devcontainer exec failed");
    }
    Ok(())
}

fn kill_session(name: &str, verbose: bool) -> anyhow::Result<()> {
    let podman_name = sanitize_podman_name(name);
    if !valid_podman_name(&podman_name) {
        anyhow::bail!("invalid session name: {}", name);
    }
    let mut cmd = Command::new("devcontainer");
    cmd.arg("down")
        .arg("--id-label")
        .arg(format!("name={}", podman_name));
    let status = run_command_verbose(&mut cmd, verbose).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("devcontainer command not found. Please install @devcontainers/cli")
        } else {
            e.into()
        }
    })?;
    if !status.success() {
        anyhow::bail!("devcontainer down failed");
    }
    println!("Killed session {}", name);
    Ok(())
}

fn list_sessions(verbose: bool) -> anyhow::Result<()> {
    let mut cmd = Command::new("devcontainer");
    cmd.arg("list");
    run_command_verbose(&mut cmd, verbose).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("devcontainer command not found. Please install @devcontainers/cli")
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

    for cmd in ["devcontainer", "git", "gh"] {
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
        assert!(err.contains("devcontainer command not found"));
        assert!(err.contains("gh command not found"));
        assert!(err.contains("config file"));
    }

    #[test]
    fn precheck_succeeds_with_all_requirements() {
        let bin_dir = tempdir().unwrap();
        for cmd in ["git", "devcontainer", "gh"] {
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
