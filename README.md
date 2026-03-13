# Beacon

**Make any repository agent-ready. Instantly.**

Beacon is a tool designed for the Web 4.0 agentic economy. It scans your codebase, infers its capabilities using AI, and generates a standards-compliant AGENTS.md manifest.

---

## Core Features

- **AI-Powered Inference:** Automatically generate AAIF-compliant [AGENTS.md](https://github.com/agentmd/agent.md) manifests.
- **Multi-provider Support:** Use Gemini or Claude with your own API keys.
- **Local Validation:** Verify your manifest for standards compliance and best practices.
- **Farcaster Bot:** Scan and validate repositories directly from Farcaster mentions.

---

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/BeaconOnBase/beacon/master/install.sh | sh
```

---

## Quickstart

**1. Generate Manifest**
```bash
export GEMINI_API_KEY=your_key
beacon generate ./my-project
```

**2. Validate Manifest**
```bash
beacon validate AGENTS.md
```

---

## Farcaster Bot

Beacon is now live on Farcaster! You can scan or validate any GitHub repository by mentioning `@beacon` in a cast.

**Commands:**
- `@beacon scan github.com/user/repo` — Generates a summary and threads the capabilities.
- `@beacon validate github.com/user/repo` — Validates an existing AGENTS.md and replies with a report.

---

## Usage

### Commands

| Command | Description |
|---|---|
| `generate` | Scans a repo and creates an AGENTS.md manifest. |
| `validate` | Checks an AGENTS.md for standards compliance. |

### Supported AI Providers
| Provider | `--provider` flag | Key |
|---|---|---|
| Gemini 2.5 Flash | `gemini` (default) | `GEMINI_API_KEY` |
| Claude | `claude` | `CLAUDE_API_KEY` |

---

## How it works

1. **Scan**: Walks the repo, extracting source files, package manifests, and OpenAPI specs.
2. **Infer**: Identifies capabilities, endpoints, and schemas using framework-aware AI inference.
3. **Generate**: Writes an AAIF-compliant `AGENTS.md` to your repo.
4. **Validate**: Ensures the manifest meets the global standard for agent discovery.
