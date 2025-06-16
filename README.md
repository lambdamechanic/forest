# Forest

Forest is an opinionated Rust tool for starting and switching between
Podman/devcontainer environments.

Each session runs on its own Git branch (named the same as the session) and
expects a remote named `origin`.

The repository is mounted inside the devcontainer but the working tree lives at
`~/worktrees/<repo>/<branch>` which is mapped to `/code` in the container.
All Git operations are handled outside the container.

## Requirements
- `gh` from GitHub
- `git`
- `podman`

The repository must include a `devcontainer.json` file. The tool searches in
the following locations (in order):

- `.devcontainer.json`
- `.devcontainer/devcontainer.json`
- `.devcontainer/<env>/devcontainer.json` when using `--devcontainer-env <env>`

The selected `devcontainer.json` must specify either an `image` field or a
`build.dockerfile` which will be built and used to launch the container.

If no configuration is found, `forest` will scaffold `.devcontainer/devcontainer.json`
using the latest Ubuntu image.

## Features
- `forest open <name> [--devcontainer-env ENV]` – open a session using the
  `devcontainer.json` from `.devcontainer/ENV` (or the default location if not
  provided). The session is created if it doesn't exist. When the container is
  running a shell is opened inside it. If the repository is not on GitHub, it is
  created under `githuborg` from the config. A local branch matching the session
  name is prepared and a remote `origin` is ensured (created with `gh repo
  create` when missing).
- `forest kill <name>` – destroy the session.
- `forest ls` – list all sessions.
- `forest precheck` – verify required tools and configuration.

## configuration

Forest reads configuration from `~/.config/forest.toml`.

## Passing credentials to the devcontainer

The devcontainer uses Goose with the `openrouter` provider. To authenticate with
OpenRouter you need to pass an API key. Set `OPENROUTER_API_KEY` in your local
environment before launching a session. `forest` uses the Dev Container CLI to
start the container, so variables defined in the `remoteEnv` section of
`devcontainer.json` (like `OPENROUTER_API_KEY`) are automatically forwarded.

Goose reads its configuration from `~/.config/goose/config.yaml`. The
devcontainer ships with a preconfigured file at this path that sets the
provider to `openrouter` and defaults to the `o4-mini` model. You can inspect
the source file at `.devcontainer/goose-config.yaml`.

```bash
export OPENROUTER_API_KEY=your-key-here
forest open my-session
echo "" | goose

## Sample Session

```bash
# start a new development session
forest open feature-xyz

# make some changes inside the container
echo "// TODO" >> src/main.rs
git commit -am "Add TODO marker"

# push the branch and open a pull request
git push -u origin feature-xyz
gh pr create --fill

# press Ctrl-D to exit the container so the next command runs on the host
# when finished, stop the session
forest kill feature-xyz
```
