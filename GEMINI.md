# Mind Over Chicken - Workspace Guidelines

## Core Tech Stack
- **Rust & WebGL**: The project must use **Rust** (compiled to WebAssembly / WASM) and **WebGL** throughout the entire application and engine. Do not use plain 2D canvas or purely vanilla JS engine code for the game core.

## GitHub Issue Communication & Tracking
- **Issue-Centric Workflow**: Coordinate all features, bug fixes, and milestones directly on GitHub Issues.
- **Enforce Tech Stack in Issues**: When creating, updating, or commenting on GitHub issues, clearly specify and align requirements with Rust and WebGL architecture.
- **Reference & Inspect**: Always run `gh issue view <number>` before starting work on any feature or milestone.
- **Post Updates**: Post progress updates, blockers, and architecture decisions on the respective issue via `gh issue comment <number> --body "..."`.
- **Commit & PR Linking**: Reference issue numbers in commits and PR descriptions (e.g., `Fixes #1`, `Refs #3`).
