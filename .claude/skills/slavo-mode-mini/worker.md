You are working on Saldo, the operator's solo side project. This file overrides `~/.claude/skills/pstack/skills/poteto-mode/worker.md` where they differ. Its rules hold even where your kickoff does not repeat them.

- If you start an agent, open its kickoff with the same two lines that opened yours. The first names poteto-mode's `worker.md`, the second this file.
- Saldo lives on GitHub and has no MRs, PRs, Jira, or labels. Drop the GitLab, `glab`, MR, title, branch, and label rules of poteto-mode's `worker.md`. Never open a PR and never push. The session that started you lands the branch.
- Commit only on the branch your kickoff names. The commit summary is a plain sentence with no Jira key. Run `cargo fmt` in `src-tauri/` before you stage, and never pass `--no-verify`.
- Never open the real database. Plain `pnpm tauri dev` opens the installed app's `saldo.db`, and so does `saldo mcp` from a plain `cargo build`. Prove a finding with `cargo test`, `pnpm test`, or a script on a temporary database. Run the app only through the `verify-saldo` skill, and only when your kickoff asks for it.
- A new worktree has no `node_modules`. Run `pnpm install` with the Bash sandbox disabled, because pnpm 12 rejects the sandbox proxy's TLS certificate. The lockfile is `pnpm-lock.yaml`. Never use npm or yarn.
- A new worktree also has no `src-tauri/libpdfium.dylib`, because it is gitignored. Copy it from `src-tauri/` in the main checkout. The macOS Tauri config bundles it, and eKasa receipt import loads it at runtime.
- Only one `tauri dev` can run on this machine, because Vite holds port 1420 with `strictPort`. That includes `verify-saldo` runs. If the port is taken, another session is running the app. Wait and retry. Never stop a run you did not launch.
