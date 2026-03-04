# EPIC-024 — Function Calls

**Summary:** Function Calls
**Stories:** STORY-0076, STORY-0077, STORY-0078
**Primary sources:** `specs/vm.md`
**Status:** 0/3 done

## STORY-0076

**Epic:** EPIC-024 — Function Calls
**Title:** Execute function calls with frame-based isolation

**As a** bootstrap pipeline
**I want** Call instructions to create a new Frame with its own values array and execute the lambda body
**So that** function calls have proper scope isolation and return values propagate correctly

**Acceptance criteria:**
- AC-1: Call instruction creates a new Frame with code set to the lambda's compiled body, a fresh values array, and locals bound from args · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0055`
- AC-2: Return instruction pops the current frame and stores the return value at the Call instruction's position in the caller's values array · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0055`
- AC-3: TailCall reuses the current frame instead of pushing a new one · impact:`local` · seam:`integration` · scenario:`SCENARIO-0055`

**Sources:**
- `specs/vm.md:210-218`
- `specs/vm.md:100-104`

**Status:** pending

## STORY-0077

**Epic:** EPIC-024 — Function Calls
**Title:** Create lambdas with captured values

**As a** bootstrap pipeline
**I want** MakeLambda to capture values from the enclosing scope by InstrIndex and bundle them with the lambda body
**So that** closures work correctly when called outside their defining scope

**Acceptance criteria:**
- AC-1: MakeLambda creates a Lambda value containing params, a reference to nested body code, and captured values read from the specified InstrIndex positions · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0059`
- AC-2: When a captured lambda is called, the captured values are available in the new frame as if they were local bindings · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0059`

**Sources:**
- `specs/vm.md:133`

**Status:** pending

## STORY-0078

**Epic:** EPIC-024 — Function Calls
**Title:** Support compiled code with nested bodies

**As a** bootstrap pipeline
**I want** CompiledCode to contain a nested array of compiled bodies for lambdas and methods
**So that** function definitions within compiled code can be referenced by index

**Acceptance criteria:**
- AC-1: CompiledCode.nested contains compiled bodies for lambdas and methods, accessible by usize index from MakeLambda and DefineMethod instructions · impact:`cross-surface` · seam:`unit`
- AC-2: MakeLambda's body field indexes into CompiledCode.nested to retrieve the lambda's compiled body · impact:`cross-surface` · seam:`unit`

**Sources:**
- `specs/vm.md:298-306`

**Status:** pending
