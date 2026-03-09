---
name: doc-coauthoring
description: Guide structured co-authoring for specs, RFCs, PRDs, decision docs, proposals, and other substantial documents so the final document is useful to real readers, not just the author.
origin: community-anthropic
---

# Doc Coauthoring

## Objective
Help the user create a strong document through structured collaboration instead
of jumping straight into a weak first draft.

This skill is for substantial documents that benefit from gathering context,
drafting section by section, and checking whether the final document works for a
fresh reader.

## When This Applies
- The user wants to write or revise a PRD, RFC, spec, proposal, design doc, or
  decision doc
- The user has a lot of context but not yet a clear document
- The document needs to be readable by other humans or future agents

## Inputs
- Document type
- Intended audience
- Desired outcome of the document
- Template or expected structure, if one exists
- Supporting context from notes, links, docs, chats, or files

## Workflow
1. Gather meta-context:
   - document type
   - audience
   - desired impact
   - template/format constraints
2. Ask the user to dump relevant context freely.
3. Read supporting artifacts only when they actually help.
4. Ask clarifying questions to close important gaps.
5. Propose or confirm the document structure.
6. Draft section by section, not all at once.
7. Refine each section through specific feedback.
8. Re-read the whole document for flow, gaps, and redundancy.
9. Do a final reader-oriented pass so the document works without hidden context.

## Decision Rules
| Situation | Action |
|---|---|
| User wants freeform writing instead | Skip rigid workflow and help directly |
| Template exists | Follow the template |
| Context is incomplete | Ask a few high-value clarifying questions |
| Section is still unclear | Brainstorm options before drafting |
| Draft is too verbose | Cut repetition and generic filler |

## Guardrails
- Do not pretend to know missing organizational context
- Do not write the whole document before understanding the audience and goal
- Prefer section-by-section refinement over large undifferentiated rewrites
- Optimize for readability by future readers, not just the current author

## Output Contract
Produce one or more of:
- document structure proposal
- section drafts
- revision suggestions
- final document pass with clarity, gap, and redundancy fixes

## Related Skills
- `final-report`
- `requirement-breakdown`
- `project-assistant`
