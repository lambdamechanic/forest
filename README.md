# forest

forest is an opinionated rust tool for starting & switching between podman/devcontainer environments.

each session has its own git branch, named the same as the session.
we assume there is a git remote called "origin".

we map the repo into the devcontainer, but start a worktree at ~/worktrees/$reponame/$branchname which is mapped to  /code inside the container which is where all changes are made
(apart from git, behind the scenes.)

## Requirements
- gh tool from github
- git
- podman

The repository must include a `devcontainer.json` file. The tool looks for:

* `.devcontainer.json`
* `.devcontainer/devcontainer.json`
* `.devcontainer/<env>/devcontainer.json` when using `--devcontainer-env <env>`

The selected `devcontainer.json` must specify an `image` field which is used
to launch the container.

If no configuration is found, `forest` will scaffold `.devcontainer/devcontainer.json`
using the latest Ubuntu image.

## Features
- forest open $name [--devcontainer-env ENV] # open the session using the
  `devcontainer.json` from `.devcontainer/ENV` (or the default location if not
  provided). The session is created if it doesn't exist. We derive the git repo
  from the current folder. If there is a git repo but no GitHub repo, create it
  under `githuborg` from the config. When opening a session we also ensure the
  `origin` remote exists (creating it with `gh repo create` when missing) and
  create a local branch matching the session name if it doesn't already exist.
- forest kill $name # destroy the session.
- forest ls # list all sessions

## configuration

~/.config/forest.toml
