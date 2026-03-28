# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in HomeRun, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, please email **<asafgallea@gmail.com>** with:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You should receive an acknowledgment within 48 hours. We will work with you to understand the issue and coordinate a fix before any public disclosure.

## Supported Versions

| Version | Supported |
| ------- | --------- |
| latest  | Yes       |

## Scope

This policy covers the HomeRun daemon (`homerund`), TUI (`homerun`), and desktop app. It does not cover the GitHub Actions runner binary itself, which is maintained by GitHub.

## Security Considerations

- **Auth tokens** are stored in the macOS Keychain, never on disk in plaintext
- **Runner processes** execute on the host machine with the current user's permissions — only register runners for repositories you trust
- **All GitHub communication** is outbound HTTPS; no inbound ports are opened
- **The daemon socket** (`~/.homerun/daemon.sock`) is accessible only to the current user via filesystem permissions
