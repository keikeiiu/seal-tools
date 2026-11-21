---
name: google-technical-writing
description: >-
  Use when writing, reviewing, or editing technical documentation, READMEs, setup guides, runbooks,
  architecture docs, or release notes for clarity, consistency, and task completion. Applies the Google
  Technical Writing One fundamentals: audience, terminology, active voice, short sentences, focused
  paragraphs, lists and tables, procedures, links, and factual accuracy. Triggers on docs review,
  "make this clearer", "improve the README", "technical writing", "documentation audit", or before
  shipping user-facing docs.
---

# Google Technical Writing review

Use this skill to review or rewrite technical documentation so the target reader can complete the task
without guessing. The source is Google's [Technical Writing One](https://developers.google.com/tech-writing/one).

## Core principle

A document succeeds when its intended reader can find the answer or finish the procedure without
re-reading, guessing an antecedent, or consulting private context. Style rules exist to serve that
outcome, not the reverse.

## Workflow

### 1. Identify the document contract

Before editing, write down:

- **Audience:** who is this for? End user, operator, contributor, maintainer?
- **Purpose:** what should they be able to do or decide after reading?
- **Type:** tutorial, how-to, reference, explanation, or release record.
- **Entry point:** what does the reader already know? What is the curse of knowledge here?

If these are missing from the document, that is a finding, not an assumption.

### 2. Read once as the target reader

Read top to bottom without fixing anything. Record:

- where you had to guess
- where you had to jump backward or forward
- commands, paths, or UI labels that do not match the current product
- claims that contradict another part of the same document or another current doc

### 3. Run the review checklist

**Scope and accuracy**

- Does the first screen state audience, purpose, and outcome?
- Are prerequisites, supported versions, and permissions explicit?
- Is there a success check? How does the reader know it worked?
- Are dates, versions, links, and release claims current?
- Is historical or superseded material labelled as history instead of current truth?

**Terminology**

- Is every acronym expanded at first use?
- Is each concept called by one name? Flag drift such as app/tool/widget/launcher, repo/repository,
  config/settings, or v3/v3.0/v3.0.x.
- Are UI labels, filenames, commands, and config keys formatted consistently?
- Are ambiguous pronouns (`this`, `it`, `they`) replaced with the noun when the antecedent is not the
  immediately preceding noun?

**Voice and sentences**

- Prefer active voice and imperative mood for procedures.
- Name the actor when it matters. If the actor is truly unknown, passive voice is acceptable.
- Target an average of roughly 15–20 words per sentence; split sentences that carry two or more ideas.
- Put the condition before the action: "If the install fails, open the log", not "Open the log if ..."
  when the condition decides the path.

**Paragraphs and sections**

- One topic per paragraph. The first sentence should lead the topic.
- Split paragraphs that mix description, procedure, and caveats.
- Use headings that describe the reader's task or question, not internal project names.
- Put key points before detail; move deep reference into a linked section.

**Lists and tables**

- Numbered lists only for ordered steps. Bulleted lists for options, conditions, or non-sequential
  facts.
- Keep list items parallel: same grammar, same level of detail, same punctuation.
- Tables for comparisons or repeated fields. Use clear headers and keep cells short.
- Do not put multi-paragraph procedures inside table cells.

**Procedures**

- One action per numbered step.
- Start with the precondition, then the action, then the expected result when it is not obvious.
- Use the exact UI label in bold and the exact command/path in code format.
- Do not assume a hidden step (download, save, restart) happened.
- Separate troubleshooting notes from the happy-path procedure.

**Links and references**

- Use descriptive link text; never "click here".
- Replace positional references ("above", "below", "earlier") with the section name or a link.
- Verify every local path, anchor, and URL.
- Do not cite a private spec from a user-facing guide without telling the reader it is a developer
  reference.

### 4. Classify the findings

Report findings in three levels:

- **P0 — blocks the task or is wrong:** broken link, missing step, contradiction, stale version,
  command that cannot work, unsupported claim.
- **P1 — causes re-reading or guessing:** undefined term, long/dense paragraph, ambiguous pronoun,
  inconsistent naming, buried prerequisite.
- **P2 — polish:** heading capitalization, link text, table alignment, wordiness.

Do not bury P0 under style notes. Fix correctness before elegance.

### 5. Rewrite

For each P0/P1, show a short before/after. Preserve facts and commands; do not invent versions, paths,
or behavior. Prefer the smallest edit that removes the ambiguity. When a whole section is structurally
wrong, rewrite the section rather than patching individual sentences.

When editing a repository:

- Update all language twins and generated references when the same fact is duplicated.
- Do not rewrite historical records, investigations, or release notes to match current behavior; label
  them as history or fix the current doc instead.
- Recheck release links and version labels after any release change.

### 6. Verify

- Read the changed doc once end to end.
- Check links, anchors, code fences, and tables in the rendered form when possible.
- Search for the old term or old version you replaced.
- Confirm the document still answers the original reader contract.

## Output contract

Use this shape for a review:

```markdown
## Verdict
One or two sentences: is the doc usable, and what is the main risk?

## P0 — correctness/blockers
| Location | Problem | Fix |

## P1 — clarity
| Location | Problem | Fix |

## P2 — polish
| Location | Problem | Fix |

## Rewrites
Before/after examples for the highest-value fixes.

## Verification
What you checked: links, versions, terminology, renders, etc.
```

## Common mistakes

- **"The reader will infer it."** If a step, actor, or prerequisite is not written, the reader is
  guessing.
- **Passive voice as politeness.** "The settings can be saved" hides who saves them and when.
- **Long sentences are efficient.** They are efficient for the writer, not the reader.
- **Style-only review.** Undefined terms and stale versions block tasks; they are not polish.
- **Editing history.** A dated record should explain what was true then, not be rewritten to today.
- **Assuming the repo is the product.** Check what the reader actually receives in the zip, installer,
  or release page.
