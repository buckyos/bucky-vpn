# Joined Node Online Status Display

- Status: complete
- Task manifest: docs/versions/v0.1/modules/vpn_web/005-joined-node-online-status/task.yaml
- Approved proposal: docs/versions/v0.1/modules/vpn_web/005-joined-node-online-status/proposal.md
- Affected paths: vpn_web/lib/joined_nodes_page.dart

## Approach
Keep the existing Status-cell behavior and add an explicit null-or-empty IP-list branch: online nodes display their addresses only when the list is non-empty, otherwise they display `online`; offline nodes continue to display `offline`.

## Risk Screen
The change is confined to one presentation expression and does not alter API parsing, backend liveness, status colors, or table interactions. The repository's `vpn_web` custom rule prevents adding a regression test by default, so verification uses targeted Flutter analysis plus direct inspection of all four relevant input states.

## Verification
- Targeted check: Windows Dart analyzer from the installed Flutter SDK: `dart.exe analyze lib\\joined_nodes_page.dart`; focused `git diff --check`
- Result: passed
- Residual risk or follow-up: Browser rendering was not launched in this environment; the supplied screenshot case is covered directly by the explicit empty-list branch. No automated regression test was added because `harness/custom-rules/vpn-web-no-new-tests-rule.md` forbids doing so without explicit user authorization.
