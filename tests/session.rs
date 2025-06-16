use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::tempdir;

const STUB_SCRIPT: &str = r#"#!/bin/sh
cmd=$1
shift
case "$cmd" in
  container)
    if [ "$1" = "exists" ]; then
      name=$2
      if [ -f "$PODMAN_STATE/$name" ]; then
        exit 0
      else
        exit 1
      fi
    fi
    ;;
  run)
    name=""
    volumes=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --name)
          name=$2
          shift 2
          ;;
        -v)
          volumes="$volumes $2"
          shift 2
          ;;
        *)
          shift
          ;;
      esac
    done
    echo "$volumes" > "$PODMAN_STATE/${name}.volumes"
    touch "$PODMAN_STATE/$name"
    exit 0
    ;;
  exec)
    name=$2
    input=$(cat)
    cd "$WORKTREE_PATH"
    sh -c "$input"
    exit 0
    ;;
  build)
    tag=""
    dockerfile=""
    context=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -t)
          tag=$2
          shift 2
          ;;
        -f)
          dockerfile=$2
          shift 2
          ;;
        *)
          context=$1
          shift
          ;;
      esac
    done
    echo "$dockerfile $context" > "$PODMAN_STATE/${tag}.build"
    touch "$PODMAN_STATE/$tag"
    exit 0
    ;;
esac
exit 1
"#;

#[test]
fn new_session_branch_inside_container() {
    let repo_dir = tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    fs::write(repo_dir.path().join("file"), "hello").unwrap();
    assert!(Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());

    let home_dir = repo_dir.path().join("home");
    fs::create_dir(&home_dir).unwrap();
    let repo_name = repo_dir.path().file_name().unwrap().to_str().unwrap();
    let worktree_path = home_dir
        .join("worktrees")
        .join(repo_name)
        .join("new-branch");

    let podman_dir = tempdir().unwrap();
    let podman_path = podman_dir.path().join("podman");
    fs::write(&podman_path, STUB_SCRIPT).unwrap();
    assert!(Command::new("chmod")
        .arg("+x")
        .arg(&podman_path)
        .status()
        .unwrap()
        .success());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forest"));
    cmd.current_dir(&repo_dir);
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            podman_dir.path().display(),
            std::env::var("PATH").unwrap()
        ),
    );
    cmd.env("HOME", &home_dir);
    cmd.env("WORKTREE_PATH", &worktree_path);
    cmd.env("PODMAN_STATE", podman_dir.path());
    cmd.arg("open").arg("new-branch");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"git branch --show-current\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("new-branch"));

    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    assert!(branch.status.success());
    assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "main");
}

#[test]
fn mounts_repo_and_worktree() {
    let repo_dir = tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    fs::write(repo_dir.path().join("file"), "hello").unwrap();
    assert!(Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());

    // create unrelated worktree which should not be mounted
    let other_wt = repo_dir.path().join("otherwt");
    assert!(Command::new("git")
        .args(["worktree", "add", "-b", "other", other_wt.to_str().unwrap()])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());

    let home_dir = repo_dir.path().join("home");
    fs::create_dir(&home_dir).unwrap();
    let repo_name = repo_dir.path().file_name().unwrap().to_str().unwrap();
    let worktree_path = home_dir
        .join("worktrees")
        .join(repo_name)
        .join("new-branch");

    let podman_dir = tempdir().unwrap();
    let podman_path = podman_dir.path().join("podman");
    fs::write(&podman_path, STUB_SCRIPT).unwrap();
    assert!(Command::new("chmod")
        .arg("+x")
        .arg(&podman_path)
        .status()
        .unwrap()
        .success());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forest"));
    cmd.current_dir(&repo_dir);
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            podman_dir.path().display(),
            std::env::var("PATH").unwrap()
        ),
    );
    cmd.env("HOME", &home_dir);
    cmd.env("WORKTREE_PATH", &worktree_path);
    cmd.env("PODMAN_STATE", podman_dir.path());
    cmd.arg("open").arg("new-branch");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"git branch --show-current\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("new-branch"));

    let volumes = fs::read_to_string(podman_dir.path().join("new-branch.volumes")).unwrap();
    assert!(volumes.contains(&format!("{}:/repo", repo_dir.path().display())));
    assert!(volumes.contains(&format!("{}:/code", worktree_path.display())));
    assert!(!volumes.contains(other_wt.to_str().unwrap()));
}

#[test]
fn builds_image_when_using_dockerfile() {
    let repo_dir = tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    fs::write(repo_dir.path().join("file"), "hello").unwrap();
    assert!(Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());

    let dev_dir = repo_dir.path().join(".devcontainer");
    fs::create_dir(&dev_dir).unwrap();
    fs::write(
        dev_dir.join("devcontainer.json"),
        r#"{ "build": { "dockerfile": "Dockerfile" } }"#,
    )
    .unwrap();
    fs::write(dev_dir.join("Dockerfile"), "FROM scratch\n").unwrap();

    let home_dir = repo_dir.path().join("home");
    fs::create_dir(&home_dir).unwrap();
    let repo_name = repo_dir.path().file_name().unwrap().to_str().unwrap();
    let worktree_path = home_dir
        .join("worktrees")
        .join(repo_name)
        .join("new-branch");

    let podman_dir = tempdir().unwrap();
    let podman_path = podman_dir.path().join("podman");
    fs::write(&podman_path, STUB_SCRIPT).unwrap();
    assert!(Command::new("chmod")
        .arg("+x")
        .arg(&podman_path)
        .status()
        .unwrap()
        .success());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forest"));
    cmd.current_dir(&repo_dir);
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            podman_dir.path().display(),
            std::env::var("PATH").unwrap()
        ),
    );
    cmd.env("HOME", &home_dir);
    cmd.env("WORKTREE_PATH", &worktree_path);
    cmd.env("PODMAN_STATE", podman_dir.path());
    cmd.arg("open").arg("new-branch");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"git branch --show-current\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("new-branch"));

    assert!(podman_dir.path().join("new-branch-image.build").exists());
}

#[test]
fn podman_name_sanitizes_branch() {
    let repo_dir = tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    fs::write(repo_dir.path().join("file"), "hello").unwrap();
    assert!(Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .status()
        .unwrap()
        .success());

    let home_dir = repo_dir.path().join("home");
    fs::create_dir(&home_dir).unwrap();
    let repo_name = repo_dir.path().file_name().unwrap().to_str().unwrap();
    let worktree_path = home_dir
        .join("worktrees")
        .join(repo_name)
        .join("feat")
        .join("cool");

    let podman_dir = tempdir().unwrap();
    let podman_path = podman_dir.path().join("podman");
    fs::write(&podman_path, STUB_SCRIPT).unwrap();
    assert!(Command::new("chmod")
        .arg("+x")
        .arg(&podman_path)
        .status()
        .unwrap()
        .success());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forest"));
    cmd.current_dir(&repo_dir);
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            podman_dir.path().display(),
            std::env::var("PATH").unwrap()
        ),
    );
    cmd.env("HOME", &home_dir);
    cmd.env("WORKTREE_PATH", &worktree_path);
    cmd.env("PODMAN_STATE", podman_dir.path());
    cmd.arg("open").arg("feat/cool");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"git branch --show-current\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("feat/cool"));

    assert!(podman_dir.path().join("feat-cool.volumes").exists());
}
