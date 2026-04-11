---
description: These are basic instructions to consider when analyzing the code or thinking about implementing a new feature
# applyTo: 'These are basic instructions to consider when analyzing the code or thinking about implementing a new feature' # when provided, instructions will automatically be added to the request context when the pattern matches an attached file
---
Always consult `implementation-rules.md` and `memory.md` as constraint and context references when planning or implementing features.
When writing PRDs or business-level planning documents, use it to ensure consistency but do NOT embed implementation details (migration DDL, struct/type definitions, file paths, API signatures) into the document. PRDs define what to build, not how.
When writing technical plans or implementation plans, include full technical details as needed.

**HARD CONSTRAINT**: NEVER USE TERMINAL TOOLS TO EDIT CONTENTS OF FILES. Always use edit tools from VS code to manipulate with files.