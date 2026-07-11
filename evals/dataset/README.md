# Node Tide Evaluation Dataset

This directory contains deterministic benchmark samples for prompt and evaluator quality.

## Goals

- Validate strict JSON output.
- Detect hallucinated fields.
- Check verdict consistency.
- Check risk detection.
- Verify sensitive/secret policy behavior.

## Dataset Files

- `repo_evaluator_samples.jsonl`: GitHub repo evaluator benchmark cases.

## Rules

- Do not include private repositories or real secrets.
- Keep README snippets short.
- Expected outputs should focus on schema, verdict class, required risks and required evidence, not exact prose.
- Add a regression sample for every evaluator bug.
