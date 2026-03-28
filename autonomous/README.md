# Nyx Autonomous Optimization Layer

Executable:

- `autonomous/nyx-autotune`

Command surface via `tools/nyx`:

- `tools/nyx analyze`
- `tools/nyx optimize auto`
- `tools/nyx profile -- <command>`
- `tools/nyx build optimize`
- `tools/nyx ecosystem health`

Safety:

- Does not modify frozen compiler core.
- Writes reports and plans under `.nyx-autotune/`.
- Optional apply mode is config-only and reversible.
