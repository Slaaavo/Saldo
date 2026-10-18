---
name: slavo-mode-mini
description: Slavo's working style for the Saldo side project. It layers on poteto-mode and lands worktree branches on master after approval in chat instead of opening PRs. Use for /slavo-mode-mini or requests to work in Slavo's style in this repo.
disable-model-invocation: true
---

# Slavo mode mini

Read `~/.claude/skills/pstack/skills/poteto-mode/SKILL.md` in full and follow it, playbooks included. The rules below win where they conflict with it.

Load every pstack skill that poteto-mode or a playbook names, such as `unslop`, `technical-writing`, `no-comments`, `how` or a `principle-*` skill, by reading `~/.claude/skills/pstack/skills/<name>/SKILL.md` and following it. Do not call the Skill tool for them. Their `disable-model-invocation` flag makes the Skill tool refuse.

This skill adapts `~/.claude/skills/slavo-mode/SKILL.md` to Saldo, a solo side project on GitHub (`origin`, trunk `master`) with no Jira, MRs, labels, or reviewers. Every "MR" in poteto-mode and its playbooks means one worktree branch, landed per Landing below. Ignore poteto-mode's GitLab, `glab`, and Cypress references.

## Business logic

- Ask the user with `AskUserQuestion` about every business-logic call the request leaves open, such as what a value means or which events count. This overrides poteto-mode's "decide and report" for business logic. Technical calls stay yours.
- Save each answer in two places. Append it to `memory.md` per `.agents/skills/remember/SKILL.md`, and skip that procedure's confirmation step, because the answer already came from the user. Also save it as a Claude project memory.
- Each answer is also a bullet in the description's `## Decisions` section.

## Worktrees

- Work in a git worktree on a new branch off `master`, so several sessions can run at once. Use `EnterWorktree`, or the worktree the app made for the session. The user also commits in the main checkout while you work.
- A new worktree has no `node_modules`. Run `pnpm install`. The lockfile is `pnpm-lock.yaml`. Never use npm or yarn.
- A new worktree also has no `src-tauri/libpdfium.dylib`, because it is gitignored. Copy it from `src-tauri/` in the main checkout. The macOS Tauri config bundles it, and eKasa receipt import loads it at runtime.
- Only one `pnpm tauri dev` can run on this machine, because Vite holds port 1420 with `strictPort`. If the port is taken, another session is running the app. Wait and retry. Never stop a run you did not launch.

## Real database

`pnpm tauri dev` opens the same `saldo.db` as the installed Saldo app on this Mac. The file is in `~/Library/Application Support/com.teamsumbal.saldo/`, or in the folder that `db-location.json` there names. The app runs pending migrations on that file at startup, before demo mode is available.

- Before you launch a branch that adds a file to `migrations/`, back up the database. While no Saldo instance runs, copy `saldo.db`, `saldo.db-wal`, and `saldo.db-shm` to the same folder with a `.bak-<YYYYMMDD-HHMMSS>` suffix. If the user's own Saldo app is open, ask them to close it. Name the backup path in your reply.
- Verify in demo mode when the change does not need real data. Demo mode runs on an in-memory database.

## Verification

- Prove every change in the running app with the `verify-saldo` skill (`.claude/skills/verify-saldo/`). It is the project's own verification skill that poteto-mode names.
- If `verify-saldo` does not exist yet, stop before Done and tell the user. It is generated with `/pstack:create-verification-skill`. Do not substitute `pnpm ci`, unit tests, or the Playwright specs in `tests/`. The specs drive `localhost:1420` in a plain browser, which has no Tauri IPC.
- `pnpm ci` must also pass. It is necessary, not sufficient.

## Extra work

- When verification turns up more bugs, fix them all in the same change.
- List every change the user did not ask for under its own heading in the reply and in the description's `## Scope`.
- When behavior changes, update `CLAUDE.md` and the `verify-saldo` skill and its feature map in the same change.
- Propose new conventions for `implementation-rules.md` per `.agents/skills/remember/SKILL.md`, which asks the user before it writes.

## Done

Done means all three:

- You proved the change works in the running app with the `verify-saldo` skill.
- The description is in chat, per Landing.
- Your reply ends with a high-level summary of the work that leads with the architectural decisions.

## Landing

This replaces poteto-mode's Opening a MR, Babysit, and Shipping playbooks. Their commit and prose rules still apply, except that commit summaries are plain sentences, not Conventional Commits.

1. Commit on the worktree branch. Follow the `commit-message` skill, but skip its JIRA key and branch-name check, because Saldo has no Jira. Saldo's husky pre-commit hook runs `lint-staged`, then `cargo fmt` and `cargo clippy -- -D warnings` in `src-tauri/`. Run `cargo fmt` there before you stage. The hook's `cargo fmt` rewrites files after staging, so its fixes stay out of the commit. Never bypass the hook with `--no-verify`.
2. Post the description in chat. It is the MR description you would have written: `## Decisions` first, then the sections from `playbooks/opening-a-pr.md`. Put the branch name and the commit list above it.
3. Wait for the user to approve in chat. Approval covers one branch. A grant of full autonomy does not approve a merge unless it says so. On requested changes, commit them and post the updated description.
4. On approval, rebase the branch onto local `master`. A worktree session cannot run git in the main checkout, so leave the worktree with `ExitWorktree` and `action: "keep"`. In the main checkout, run `git merge --ff-only <branch>` and `git push origin master`. Then run `git worktree remove <path>` and `git branch -d <branch>`. If the session cannot leave its worktree, give the user these commands.
5. If the fast-forward fails because `master` moved, rebase again. If it fails because of the user's uncommitted changes in the main checkout, stop and tell the user. Never stash or discard their work.

### Decisions

List each choice that the user cannot predict from the request and the diff. That includes a business rule settled in the session, a request read one way when it allows two, a departure from a plan in `plans/`, and a removal that looks like a regression. For each one, say what the code does, what the user would expect instead, and why. Give the reason, or who decided when it is a product call. "As decided on <date>" is neither. Collect the bullets from the session's `AskUserQuestion` answers, the user's corrections, and your own departures. Do not repeat them under `## Scope`. Drop the section when nothing qualifies.

## Posting

- Post to GitHub (issues, pull requests, releases, comments) only when the request says to. Otherwise put the draft in chat.

## Explaining

- The user often has not read the code. Lead with the result or the impact, then the mechanics.
- Put the result first. Describe how you got there only when the user asks, and after the result.

## Estimates

- Never estimate time in hours, days, weeks, or sprints. This holds when a skill asks for effort, such as figure-it-out's scope step.
- Size the work by its scope instead. Name what changes and where, how many call sites, files, or migrations it touches, and what is risky or unknown.
