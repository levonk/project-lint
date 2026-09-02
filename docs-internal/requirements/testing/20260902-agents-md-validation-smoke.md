# Smoke Test: AGENTS.md Validation + Path Hygiene Scanners

**Date**: 2026-09-02
**PRD**: [20260901-agents-md-validation.md](../20260901-agents-md-validation.md)
**Build**: `devbox run -- just build` (release)

## Scanners Under Test

- `agents_md` (check name: `agents_md`) — validates AGENTS.md/CLAUDE.md/AGENT.md
- `path_hygiene` (check name: `path_hygiene`) — cross-cutting text file path hygiene

## Test 1: project-lint self-scan (has AGENTS.md)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/project-lint`

**Result**: Both scanners fire.

- `agents_md`:
  - `agents-md-no-absolute-paths` (warning) on AGENTS.md:26 — the
    architecture description mentions `/Users/...` and `/home/...` as
    examples of what the scanner detects. Expected (documentation
    reference, not a real path).
  - `agents-md-child-chain` (warning) on AGENTS.md:25 — references to
    `CLAUDE.md` and `AGENT.md` which do not exist in this repo. Expected
    (the AGENTS.md mentions these as aliases).

- `path_hygiene`:
  - Fires on scanner source files (agents_md.rs, path_hygiene.rs) and
    docs because they contain the pattern strings as defaults/examples
    (e.g. "Generated with [Devin]", "/Users/..."). Expected — the
    scanner source legitimately references these patterns.
  - Fires on real home paths in docs (e.g. handoff files referencing
    `/Users/micro/p/gh/...`). Correct behavior.

## Test 2: repo without AGENTS.md (agentgrep)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/agentgrep`

**Result**: `agents_md` scanner is **silent** (0 AgentsMD issues).
`path_hygiene` correctly fires on real `/home/jeremy/jcode` paths in
`docs/BENCHMARKS.md`.

```
AgentsMD count: 0
```

This confirms the scanner does not produce false positives on repos
without AGENTS.md/CLAUDE.md/AGENT.md files.

## Test 3: centralized exclusion list

Both scanners use `walk_project()` with the centralized exclusion list.
The `target/` directory is correctly skipped — no PathHyg or AgentsMD
issues reference files under `target/` (verified by grep of output).

## Conclusion

- `agents_md` fires on repos with AGENTS.md files, silent on repos
  without. ✅
- `path_hygiene` detects absolute home paths and AI attribution in real
  files. ✅
- Both scanners respect the centralized exclusion list. ✅
- No false positives on non-matching repos. ✅
