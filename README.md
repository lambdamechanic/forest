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

## Features
- forest open $name # open the session if it exists: otherwise, start a session named $name as well as a github branch & a local branch. assumes "origin" is the remote. we derive the git repo to use from the folder we're currently in. if there is a git repo but no github repo, create it as the name of the folder on the organisation listed as "githuborg" in the config file.
- forest kill $name # destroy the session.
- forest ls # list all sessions

## configuration

~/.config/forest.toml
