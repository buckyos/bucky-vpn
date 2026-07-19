# [Task Name]

## Feature / Stage
- Workflow Tier: high-risk
- Feature:
- Stage:
- Stage Responsibility:
- Version:
- Packet Module:
- Target Module:
- Task Packet: `docs/versions/<version>/modules/<project-or-globals>/<task-seq>-<task-slug>/`
- Submodule:
- change_id:
- Owner:
- Parent Task:
- Depends On:

## Goal
- What this task must finish.

## Assumptions And Ambiguities
- Assumptions:
- Ambiguities:
- Decision or return route:

## Success Criteria
- Criterion 1:
- Criterion 2:
- Verification signal:

## Inputs
- Upstream artifacts:
- Relevant docs:
- Relevant code:
- Constraints:

## Entry Checks
- [ ] This generic downstream staged-task template is used only for confirmed high-risk work; lower tiers still retain their common `task.yaml` plus approved `proposal.md`, and standard additionally uses `docs/changes/<change>.md`
- [ ] If this task may affect code, tests, runtime behavior, UI behavior, build behavior, bugfixes, optimization, or refactoring, `harness/rules/task-entry-gate-rules.md` was applied first
- [ ] Required upstream documents exist
- [ ] Required upstream approvals exist
- [ ] The changed-path manifest contains only the active stage's artifacts; needed cross-stage work was returned or split
- [ ] Filesystem access is not granted by stage metadata; stage-scope completion nevertheless rejects out-of-stage changed paths
- [ ] If this is an implementation or bugfix task, active `version`, packet `module`, `target_module`, and `change_id` are explicit
- [ ] If packet module is `globals`, implementation scope checks use `--target-module <project>` independently for each affected project
- [ ] If this targets a direct submodule packet, active `submodule` is explicit
- [ ] If this is an implementation or bugfix task, a current schema result exists for the active packet; unchanged schema inputs were not rechecked
- [ ] If this is an implementation or bugfix task, planned impact and actual impact are traceable without treating Scope Paths as an allowlist
- [ ] If this is a cross-module implementation or bugfix task, affected modules have useful proposal/design coverage
- [ ] If this is a cross-submodule implementation or bugfix task, every affected submodule packet has direct scope mapping
- [ ] If run, the preparation profile is treated as evidence rather than edit authorization

## Work
- What should be produced.
- What must be validated.
- What should be left for the next task.

## Steps
### Step 1
- Action:
- Skill or tool:
- Output:
- Verify:

### Step 2
- Action:
- Skill or tool:
- Output:
- Verify:

### Step 3
- Action:
- Skill or tool:
- Output:
- Verify:

## Deliverables
- Deliverable 1:
- Deliverable 2:

## Done Criteria
- [ ] Goal is met
- [ ] If this is a testing task, generated or changed tests are reachable through `harness/scripts/test-run.py`
- [ ] Required validation has a current passing result; unchanged inputs were not rerun
- [ ] No checker rejected work solely because of the project files that were read or changed
- [ ] Residual risks are recorded

## Next-Stage Gate
- [ ] Preconditions for the next stage are satisfied
- [ ] If the next stage is implementation, manual flow has approved proposal/design; auto-pipeline has a launch-confirmed proposal plus validated pipeline-plan mappings and `Scope Paths`
- [ ] If the next stage is implementation, those approved docs already contain the next task's required content

## Return Routing On Failure
- Return to stage:
- Reason:
- Blocking or non-blocking:

## Expected Impact
- Likely files:
- Additional files may be read or changed as needed; Harness imposes no repository path restrictions.
