# Sprint 6 — Memory UI, hardening, release notes

## Goal

Make the last sprint's work visible, tighten what the real-model runs exposed, and leave `dev` ready to go to `main`.

## Scope

1. **Dreams in the UI.**
   - Protocol: the resident's agent view carries recent dreams (day, text, number of insights, number let go).
   - Info panel: a 「梦」 section; insights are labelled in 「记着的事」.
   - Diary: the night's dream shown at the start of the day it was dreamt.
2. **Hardening from the real runs.**
   - Dream material: procedural mode notices ("轮到你…") rank below what people actually said.
   - Story prompt: scenes only for confrontations — cap at half the beats in `tidy`.
   - Live check: one resident sleeps through a night in a running town with the real model, if the key is still valid.
3. **Regression across features.** Default town with no mode: saves and prompts unchanged (existing tests). Scripted towns still start. Hide/show still works.
4. **Docs.** Player-facing `README.md` section for the three features; wiki pass; final sprint report with what is left.

## Acceptance

- Wire sample with dreams compiled against `protocol.ts`.
- Agent test: dream material prefers speech over procedural notices.
- Story parser test: surplus scenes are dropped.
- Browser: dream section renders for a resident who has dreamt.
- Full local gate green; PR merged into `dev`.

## Out of scope

Merging `dev` into `main` — that is the product owner's call.
