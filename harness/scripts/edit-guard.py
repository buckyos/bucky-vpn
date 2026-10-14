#!/usr/bin/env python3
"""Retired execution-mode entrypoint; retained only to explain migration."""
import sys
def main() -> int:
    print("edit-guard: retired; use task.yaml, harness-check.py and task-transition.py with standard/high-risk workflows", file=sys.stderr)
    return 1
if __name__ == "__main__":
    raise SystemExit(main())
