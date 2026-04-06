---
name: remember
description: 'Update project memory and implementation rules after completing work. Use when asked to "remember", "save what we learned", "update memory", "update rules", "capture learnings", or at the end of a multi-step implementation session to persist insights. Also auto-invoked after completing significant multi-step implementations. Writes to memory.md and implementation-rules.md at the project root.'
---

# Remember

Persist project knowledge into two root-level files after completing work. This keeps future sessions informed about what was built, why, and how.

## When to Use

- After completing a feature implementation or significant change
- When the user says "remember", "save learnings", "update memory", or "update rules"
- At the end of a multi-step session where new patterns, decisions, or conventions were established
- When a workaround, gotcha, or non-obvious behavior was discovered

## Target Files

| File | Purpose | Content type |
|------|---------|-------------|
| `memory.md` | What was done and why | What was built, why decisions were made, how features relate to each other, domain context, gotchas encountered |
| `implementation-rules.md` | How to implement features correctly | Coding conventions, patterns, architectural rules, API design, data model semantics, technical constraints |

## Procedure

### Step 1 — Read Current State

Read both files in full to understand what is already captured:
- `memory.md` (project root)
- `implementation-rules.md` (project root)

### Step 2 — Identify New Knowledge

Review the conversation history and recent changes. Extract:

**For `memory.md`** (narrative context — what and why):
- What was built or changed and the motivation behind it
- Why a particular approach was chosen over alternatives
- How features connect to or depend on each other
- Domain concepts and business rules that inform the design
- Gotchas, surprises, or non-obvious behaviors discovered
- Context that would help a future session understand the project's history

**For `implementation-rules.md`** (technical rules — how):
- Coding conventions and patterns (e.g., "always use X for Y")
- API design patterns (command signatures, error handling approaches)
- Component or module structure rules
- Data model changes or new tables/columns and their semantics
- Workarounds with specific technical steps
- Testing patterns or requirements
- Migration or deployment procedures

### Step 3 — Present Candidates for Confirmation

Before writing anything, present the extracted candidates to the user for review. Format them as a numbered list grouped by target file:

**Proposed additions to `memory.md`:**
1. Brief description of the candidate entry
2. Brief description of the candidate entry

**Proposed additions to `implementation-rules.md`:**
1. [Section: §9] — Brief description of the candidate rule
2. [Section: §8] — Brief description of the candidate rule

**Proposed updates (replacing existing content):**
1. [file] — What changes and why

Ask the user to confirm, remove items, or suggest edits before proceeding. Only write the entries the user approves.

### Step 4 — Deduplicate and Merge

- Do NOT duplicate information already present in either file
- If an existing entry needs updating (e.g., a rule was refined), update it in place
- If a section doesn't exist yet for the new content, add an appropriate section
- Keep entries concise — bullet points or short paragraphs, not essays

### Step 5 — Write Approved Updates

Edit each file, adding only the user-approved entries.

**For `memory.md`**: Use a **dated log format**:
- Each entry is preceded by a date heading: `## YYYY-MM-DD`
- If multiple entries share the same date, they appear as bullets under one heading.
- **Append new entries at the bottom** of the file — the log is chronological, oldest first, newest last.
- Each entry captures *what* was done and *why*, or how things relate. Write concisely — bullet points, not prose.
- Do NOT reorder existing entries when adding new ones.

**For `implementation-rules.md`**: Follow the existing numbered section structure. Add new rules (patterns, conventions, workarounds with technical steps) to the relevant section, or create a new numbered section if needed.

### Step 6 — Enforce Size Limit

Each file must stay under **10,000 tokens** (~7,500 words). Only compact when the file is actually approaching this limit — do not compact proactively.

When compaction is needed for `memory.md`:
1. Leave the most recent dated entries untouched in their dated log format.
2. Starting from the oldest entries, group multiple `## YYYY-MM-DD` headings into combined headings (e.g., `## 2026-03-10 – 2026-03-14`) until enough space is recovered.
3. Within each grouped heading, re-organise bullets by **topic** (e.g., "Accounts", "Buckets", "Testing").
4. Merge related bullets into single denser points; remove entries superseded by later decisions.
5. Never archive to separate files — keep everything in `memory.md`.

When compaction is needed for `implementation-rules.md`:
1. Merge related rules into single, denser bullet points.
2. Remove rules that are now obvious from the codebase or were superseded.
3. Prioritise recent and frequently-relevant rules.

### Step 7 — Confirm

Summarize what was written to each file so the user has a final record of what was persisted.

## Quality Criteria

- **memory.md is about what and why, not how**: No code patterns, no struct definitions, no step-by-step technical recipes. Those belong in `implementation-rules.md`.
- **memory.md is a dated log**: New entries always append at the bottom under a `## YYYY-MM-DD` heading. Never insert entries above existing ones.
- **implementation-rules.md is about how, not why**: It captures patterns and conventions. Rationale belongs in `memory.md`.
- **No overlap**: If something is in one file, it should not be repeated in the other. Cross-reference instead (e.g., "see implementation-rules.md §7 for migration details").
- **No vague entries**: Every entry should be actionable or informative. "We decided to use X" is only useful with "because Y".
- **No stale content**: If a decision was reversed or a workaround is no longer needed, remove or update the entry.
- **Consistent tone**: Both files use impersonal, concise, technical writing. No narrative prose.
