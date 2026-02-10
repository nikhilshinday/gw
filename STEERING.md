# Steering (Human Preferences)

This file captures durable steering from the human so it can be applied consistently across tasks.

## Interaction Preferences

- Log and consolidate even soft preferences/suggestions: if I say “prefer”, “maybe”, or make a suggestion, treat it as steering worth capturing.

## Engineering Preferences

- TBD

## Repo Conventions

- Versioning: when pushing changes meant to be shared (release-worthy), bump the semver in `Cargo.toml` (patch by default unless behavior warrants minor/major), and align any tags/releases/Homebrew artifacts to the same version.
- `gw new` UX: avoid extra syntax. Treat input as either a branch name or (only) a GitHub PR URL. For remotes, do not special-case `origin`: if exactly one remote exists use it, otherwise prompt the user to choose.

## Don’t Do

- TBD

## Examples

- TBD
