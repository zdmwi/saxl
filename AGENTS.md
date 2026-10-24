# Development Rules

## Conversational Style

- Keep answers short and concise
- No emojis in commits, issues, PR comments, or code
- No fluff or cheerful filler text (e.g., "Thanks @user" not "Thanks so much @user!")
- Technical prose only, be direct
- Use concise, clear, simple language. Define unavoidable jargon before using it.
- Explain non-trivial designs and problems as: problem, concrete example or short trace, then solution. State why the solution is necessary and distinguish it from optional complexity.
- Prefer concrete behavior and small illustrations over abstract summaries, dense terminology, or unexplained lists of changes.
- When the user asks a question, answer it first before making edits or running implementation commands.
- When responding to user feedback or an analysis, explicitly say whether you agree or disagree before saying what you changed.

## Branch naming

Branch names follow the [Conventional Branch](https://conventionalbranch.org/)
specification: `<type>/<description>`, lowercase, hyphen-separated.

Types: `feature/` (or `feat/`), `bugfix/` (or `fix/`), `hotfix/`, `release/`, `chore/`.
`main`, `master` and `develop` take no prefix.

Examples: `feature/add-login-page`, `bugfix/fix-header-overflow`, `release/v1.2.0`.

When you create a branch yourself rather than at a person's direction, prefix it with
your own agent name instead — `claude/`, `codex/`, `copilot/`, `cursor/`, or the
vendor-neutral `ai/` — so the branch records who wrote it.

## Updating branches

Prefer rebasing over merging. To bring a branch up to date with `main`, run
`git rebase origin/main` and push with `git push --force-with-lease`, instead of
merging `main` into the branch. This keeps history linear and keeps merge commits
out of pull requests.

## Commit Conventions
Follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification for all git commit messages:

Format: <type>[optional scope]: <description> [optional body] [optional footer(s)]

Allowed Types:
- feat: New feature
- fix: Bug fix
- docs: Documentation changes
- style: Code style (formatting, semicolons, etc.)
- refactor: Code refactoring without feature/fix
- perf: Performance improvements
- test: Adding or updating tests
- build: Build process or external dependencies
- ci: CI configuration changes
- chore: Build process, dependencies, tooling

Rules for the agent:
- Use imperative mood in the summary (e.g., "add feature" not "added feature").
- Keep the first line under 72 characters.
- Never include `Co-authored-by` or `Generated-by` AI trailers unless requested.
- Validate messages using: `echo "type(scope): description" | npx commitlint`.

