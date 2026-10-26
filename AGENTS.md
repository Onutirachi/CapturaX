# CapturaX — Agent Guide

CapturaX is a desktop app (Windows and Linux) to watch and hear capture cards with the lowest
possible latency, capture screens and windows, take screenshots, record, and stream to friends
by link. Read [the constitution](.specify/memory/constitution.md) before doing anything: it
defines the principles, scope and workflow, and it overrides this guide.

## Status

Early rewrite. There is no application code yet. The previous version lives in the
[CapturaX-old](https://github.com/Onutirachi/CapturaX-old) repository and is reference only.

## Starting a session

1. Read the constitution.
2. Find the spec you are working on in `specs/<timestamp>-<slug>/` (the branch name
   `feat/<timestamp>-<slug>` points to it).
3. Read its `handoff.md` first, then `spec.md`, `plan.md` and `tasks.md` as needed. Do not read
   other specs unless the handoff says so.
4. At the end of the session, update `handoff.md` (template:
   `.specify/templates/handoff-template.md`).

## Rules that are easy to break

- **No code before the spec is approved** by the maintainer. Ask for explicit confirmation
  before writing any implementation code.
- **Never commit without the maintainer validating the change**, and **never push** unless
  asked. Show what changed and wait.
- Conventional Commits. **No `Co-Authored-By` or any AI attribution trailer.**
- Everything in the repository is in English. Talk to the maintainer in the language they
  use (currently Portuguese).
- A correction to an unmerged commit is folded into it (amend or fixup), not added as a new
  "fix" commit and never reverted.
- If a decision changes during implementation, update the spec before the code.
- Experiments stay on their own branch; their code is not merged unless a spec adopts it.

## Spec workflow (Spec Kit)

| Step | Skill | Output |
|---|---|---|
| Create the spec and its branch | `/speckit-specify` | `spec.md`, branch `feat/<timestamp>-<slug>` |
| Resolve open questions | `/speckit-clarify` | updated `spec.md` |
| Technical design | `/speckit-plan` | `plan.md` (+ research, data model, contracts) |
| Break into tasks | `/speckit-tasks` | `tasks.md` |
| Check consistency | `/speckit-analyze` | report |
| Implement (only after approval) | `/speckit-implement` | code |

Spec Kit auto-commit hooks are disabled on purpose; commits happen only after validation.
