# Understanding GitHub Actions Self-Hosted Runners

This guide explains how GitHub Actions self-hosted runners work under the hood. If you're new to self-hosted runners or want to understand what HomeRun automates for you, start here.

## Table of Contents

- [How Runners Communicate with GitHub](#how-runners-communicate-with-github)
- [The Manual Setup Process](#the-manual-setup-process-what-youd-do-without-homerun)
- [Runner Lifecycle](#runner-lifecycle)
- [Permissions and Access](#permissions-and-access)
- [Public vs. Private Repositories](#public-vs-private-repositories)
- [Runner Levels: Repo, Org, Enterprise](#runner-levels-repo-org-enterprise)
- [What HomeRun Automates](#what-homerun-automates)

## How Runners Communicate with GitHub

All communication is **outbound HTTPS** from your machine to GitHub. GitHub never connects inbound to your runner — no open ports, no firewall rules, no tunnels.

```
Your machine                            GitHub
┌──────────────┐                      ┌──────────────────┐
│              │                      │                  │
│  Runner app  │ ── HTTPS long ────▶  │  Message Queue   │
│  (run.sh)    │    poll (outbound)   │  (per runner)    │
│              │                      │                  │
│              │ ◀── encrypted ────── │  Workflow Manager │
│              │     job message      │                  │
└──────────────┘                      └──────────────────┘
```

The runner uses **HTTP long polling** to listen for jobs:

1. The runner opens an HTTPS request to its dedicated message queue on GitHub
2. The request **stays open**, waiting for a message (this is long polling — not WebSocket, not periodic short polling)
3. If no job arrives before the timeout, the connection closes and immediately reopens
4. This loop is what produces the `"Listening for Jobs"` log message

When a matching job is found, GitHub's Workflow Manager places an **encrypted message** in the runner's queue. The long poll returns with the job payload, and the runner begins execution.

## The Manual Setup Process (What You'd Do Without HomeRun)

To understand what HomeRun automates, it helps to see what GitHub asks you to do manually. When you go to **Settings > Actions > Runners > New self-hosted runner** in a repository, GitHub shows three steps:

### Step 1: Download the runner application

The runner is a standalone application published at [github.com/actions/runner](https://github.com/actions/runner). GitHub provides platform-specific tarballs:

```sh
# Create a folder
mkdir actions-runner && cd actions-runner

# Download the latest runner package (example: macOS x64)
curl -o actions-runner-osx-x64-2.333.0.tar.gz -L \
  https://github.com/actions/runner/releases/download/v2.333.0/actions-runner-osx-x64-2.333.0.tar.gz

# Optional: validate the hash
echo "2b0ba7df7be9b9c36b4b86c19539b3a8be027ce926610b71606a6e4451970946  actions-runner-osx-x64-2.333.0.tar.gz" | shasum -a 256 -c

# Extract
tar xzf ./actions-runner-osx-x64-2.333.0.tar.gz
```

You pick the **OS** (macOS, Linux, Windows) and **architecture** (x64, ARM64). The tarball contains the runner binary, `config.sh`, `run.sh`, and supporting libraries.

### Step 2: Configure (register with GitHub)

```sh
./config.sh --url https://github.com/<owner>/<repo> --token ABIR3G7PCZST245ZQ25Y5UDJYVL6U
```

This registration token is **time-limited** (expires in 1 hour) and is shown on the GitHub settings page. The `config.sh` script:

1. Generates an **RSA key pair** — private key stays local, public key sent to GitHub
2. Registers the runner with GitHub's service, receiving a `clientId`
3. Writes credentials to hidden files in the runner directory:
   - `.credentials` — OAuth connection info (client ID, auth URL)
   - `.credentials_rsaparams` — RSA private key parameters (the sensitive file)
   - `.runner` — runner metadata (name, server URL, runner ID)

On macOS/Linux, the private key is protected by **file permissions** (`chmod 600`). On Windows, it uses DPAPI encryption.

### Step 3: Run

```sh
./run.sh
```

This starts the long-polling loop. The runner connects to GitHub, authenticates with its private key, and begins listening for jobs.

### Step 4: Use in a workflow

```yaml
# In your .github/workflows/*.yml
runs-on: self-hosted
```

> **HomeRun automates all of the above.** It downloads and caches the runner binary, fetches registration tokens via the GitHub API, runs `config.sh` and `run.sh`, and monitors the process — all from a single click or API call.

## Runner Lifecycle

### 1. Registration (one-time setup)

When a runner is first configured (either manually via `config.sh` or automatically by HomeRun), the following happens:

1. A **time-limited registration token** is generated (expires in 1 hour)
2. The runner generates an **RSA key pair** locally
3. The **private key** stays on your machine in `.credentials_rsaparams` (protected by file permissions)
4. The **public key** is sent to GitHub, which assigns a unique `clientId` to this runner
5. Configuration is stored locally in `.runner` and `.credentials` files

### 2. Startup

Each time the runner process starts:

1. It loads its private key from disk
2. Requests an **OAuth token** from GitHub's Token Service using the key
3. This token grants access to the runner's dedicated message queue
4. The runner enters the long-polling loop, waiting for jobs

### 3. Job Execution

When a workflow run is triggered and matches this runner's labels:

1. GitHub's Workflow Manager encrypts the job with the **runner's public key** — only this runner can decrypt it
2. The encrypted message is placed in the runner's queue
3. The long poll returns with the message; the runner decrypts it
4. The job payload includes a **short-lived OAuth token** (`GITHUB_TOKEN`) scoped to the job (duration + 10 min, max 6 hours)
5. The runner clones the repo, executes workflow steps, and reports status back via HTTPS
6. When done, the runner returns to listening for the next job

### 4. Deregistration

When a runner is removed:

1. The runner calls GitHub to deregister itself
2. GitHub removes the runner from its registry and deletes the associated public key
3. Local credentials are cleaned up

## Permissions and Access

### Who Can Register Runners

| Level                     | Required Permission         |
| ------------------------- | --------------------------- |
| **Repository (personal)** | Repository **owner**        |
| **Repository (org)**      | Repository **admin** access |
| **Organization**          | Organization **owner**      |
| **Enterprise**            | Enterprise **owner**        |

### Who Can Use Runners in Workflows

Anyone with **write access** to a repository can author a workflow file with `runs-on: self-hosted`. However, for the job to actually run, a runner must be:

1. Registered to that repository, **or**
2. Part of an org/enterprise runner group that includes that repository

### Common Gotcha: "I can't use self-hosted runners on a repo I don't own"

This is a frequent source of confusion. Four things must align for a job to land on a self-hosted runner:

```
Runner registration:  Admin/Owner only  → "who can plug in machines"
Runner group access:  Org owner only    → "which repos can use which machines"
Workflow authoring:   Write access       → "who can write runs-on: self-hosted"
Job execution:        Automatic          → "GitHub matches labels, queues the job"
```

If you have write access to a repo but not admin, you can push a workflow with `runs-on: self-hosted`, but:

- You **can't register a runner** against that repo (the Settings > Actions > Runners page requires admin)
- If the org has org-level runners, the **org owner** controls which repos can access them via runner groups
- If the repo isn't in any runner group's allow-list, no runner will pick up your job

**This is why forking works** — when you fork a repo, you become the **owner** of the fork. As owner you can register runners against it, and your existing runners become available.

## Public vs. Private Repositories

> **GitHub's official guidance: self-hosted runners should almost never be used with public repositories.**

### Why Public Repos Are Risky

Anyone can fork a public repo, modify the workflow YAML, open a pull request, and that PR's workflow could run on **your** self-hosted runner. This means arbitrary code execution on your machine with access to:

- Your local filesystem
- Environment variables and secrets
- Network and cloud metadata services (AWS IMDS, Azure IMDS, etc.)
- Any tools or credentials on the machine

### Private Repos Are Safer, But Not Risk-Free

For private repos, the attack surface is smaller since only users with **read access** can fork and open PRs. Still, consider that anyone with repo access could potentially run untrusted code on your runner.

### Default Behavior

GitHub runner groups only allow **private repositories** by default. Public repo access must be explicitly enabled by an org owner.

### Recommendations

- Use self-hosted runners only with **private** repositories
- If you must use them with public repos, configure workflow approval for fork PRs (`Settings > Actions > Fork pull request workflows`)
- Consider using **ephemeral runners** that are destroyed after each job for better isolation
- Limit the permissions of the `GITHUB_TOKEN` in your workflows

## Runner Levels: Repo, Org, Enterprise

Runners can be registered at three levels:

| Level            | Scope                                   | Use Case                                 |
| ---------------- | --------------------------------------- | ---------------------------------------- |
| **Repository**   | Single repo only                        | Dedicated runners for a specific project |
| **Organization** | Shared across selected repos in the org | Team-wide runner pools                   |
| **Enterprise**   | Shared across multiple orgs             | Company-wide infrastructure              |

**Organization and enterprise runners** are managed via **runner groups**, which control:

- Which repositories can send jobs to which runners
- Whether public repos are allowed (default: no)
- Which users/teams can access the group

## What HomeRun Automates

HomeRun handles the entire runner lifecycle so you don't have to run shell scripts or manage tokens manually:

| Manual Process                            | HomeRun Equivalent                                      |
| ----------------------------------------- | ------------------------------------------------------- |
| Generate registration token via GitHub UI | Automatic — uses GitHub API via Device Flow auth        |
| Download runner binary + extract tarball  | Automatic — downloads, caches, and copies per runner    |
| Run `config.sh` with token and options    | Automatic — configures with repo, name, and labels      |
| Run `run.sh` or install as service        | Automatic — daemon spawns and monitors the process      |
| Monitor runner status in GitHub UI        | Live dashboard — TUI, desktop app, or CLI               |
| Tail log files manually                   | Real-time log streaming via WebSocket                   |
| Restart crashed runners                   | Auto-restart with configurable retry (up to 3 attempts) |
| Deregister via `config.sh remove`         | Automatic on runner deletion                            |
| Scale up: repeat all above N times        | Batch creation — specify count, HomeRun does the rest   |
| Check system resources                    | Built-in CPU/RAM/disk monitoring per runner             |

## Further Reading

- [GitHub Docs: About self-hosted runners](https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners/about-self-hosted-runners)
- [GitHub Docs: Security hardening for GitHub Actions](https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions)
- [GitHub Docs: Adding self-hosted runners](https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners/adding-self-hosted-runners)
- [GitHub Actions Runner source code](https://github.com/actions/runner)
- [HomeRun Architecture](ARCHITECTURE.md)
