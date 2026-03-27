# React Test Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Vitest test coverage for the React/TypeScript desktop app
(components + hooks), integrate coverage into CI and badge workflow.

**Architecture:** Mock at the `api/commands.ts` boundary (the thinnest layer over
Tauri `invoke`). All hooks and components above it run as real code. Shared test
factories provide consistent test data.

**Tech Stack:** Vitest 4, React Testing Library 16, @vitest/coverage-v8, jsdom

**Worktree:** `.worktrees/feat-react-tests` (branch `feat/react-test-coverage`)

**All commands run from:** `apps/desktop/` within the worktree unless stated otherwise.
