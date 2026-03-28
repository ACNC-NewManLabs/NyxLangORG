# nyx-collab CLI

Commands:

- `ecosystem-insights`
- `help-solve --query <text>`
- `discover-projects [--skill <skill>] [--tag <tag>]`
- `contribute --alias <name> --skills <csv> --contact <hint>`
- `share-insight --insight-type <type> --pattern-file <file> --recommendation <text> --impact <0..1>`
- `publish-knowledge --kind <kind> --title <title> --body-file <file> --tags <csv> --author <alias>`
- `analytics`

Privacy:

- Insight sharing requires opt-in (`--opt-in` or `NYX_COLLAB_OPT_IN=true`).
- Patterns are anonymized before upload.
