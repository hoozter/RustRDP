# Security and privacy

## Credentials

RustRDP never serializes passwords. If **Save password** is enabled, a random
credential identifier is stored in the profile while the secret is stored by a
Secret Service provider such as KWallet or GNOME Keyring. At launch, the secret
is retrieved only long enough to write it to FreeRDP through
`/from-stdin:force`, then the Rust string is zeroed.

Connection backups exclude both secrets and saved-password preferences. Process
arguments, application logs and UI diagnostics must never contain a password.

## Configuration

The normal file is `~/.config/rustrdp/config.toml`. Writes use a temporary file,
flush it, atomically rename it and request owner-only permissions (`0600`). The
schema is versioned and a newer unknown schema is rejected instead of guessed.

Profiles still contain connection metadata such as hostnames and usernames.
Treat configuration files and exported backups as private even though they do
not contain passwords.

## Remote identity

All sessions use FreeRDP's trust-on-first-use certificate policy. The first
identity is accepted and remembered; a changed identity is rejected. TOFU does
not prove that the first connection was safe. Verify important hosts through an
independent channel and investigate every later change.

## Local integration

The controller listens only on loopback, uses a new random route for each
session, exposes no secrets and accepts only fixed actions. KWin scripts select
only the known child PID and unload themselves after one operation.

Folder, clipboard, printer, audio and microphone redirection cross the trust
boundary into the remote Windows computer. Enable only what the session needs.

## Diagnostics

FreeRDP stderr is drained to prevent child-process deadlock, retained only up to
a fixed bound and redacted for credential-like lines before display. Technical
output can still contain hostnames and environment details; review it before
sharing publicly.

## Reporting a vulnerability

Do not include passwords, wallet exports, private hostnames or live controller
URLs in a public issue. Once the GitHub repository exists, use its private
security-advisory feature for sensitive reports. Ordinary non-sensitive bugs may
use the issue tracker.

## Limits

RustRDP cannot secure a compromised desktop, wallet, FreeRDP binary, remote
computer or network. It does not sandbox FreeRDP. Distribution packages and
build artifacts should therefore come from a trusted source and be reproducible
from the public repository revision.
