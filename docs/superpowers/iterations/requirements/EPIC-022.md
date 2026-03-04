# EPIC-022 — Indexed RPN Execution

**Summary:** Indexed RPN Execution
**Stories:** STORY-0070, STORY-0071, STORY-0072, STORY-0073
**Primary sources:** `specs/vm.md`
**Status:** 0/4 done

## STORY-0070

**Epic:** EPIC-022 — Indexed RPN Execution
**Title:** Execute indexed RPN bytecode with per-instruction value storage

**As a** bootstrap pipeline
**I want** the VM to execute compiled bytecode where each instruction stores its result at values[ip] and operands reference previous results by InstrIndex
**So that** compiled FMPL code runs correctly without an operand stack

**Acceptance criteria:**
- AC-1: Each instruction stores its computed result in values[ip] where ip is the instruction's position in the code array · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0053`
- AC-2: Binary instructions (Add, Sub, Mul, Div, Mod) read operands from values[lhs] and values[rhs] where lhs and rhs are InstrIndex references · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0053`
- AC-3: Unary instructions (Neg, Not) read operand from values[operand] where operand is an InstrIndex reference · impact:`local` · seam:`unit` · scenario:`SCENARIO-0053`
- AC-4: The expression (3 + 4) * 5 compiles to LoadInt(3), LoadInt(4), Add(0,1), LoadInt(5), Mul(2,3) and evaluates to 35 · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0053`

**Sources:**
- `specs/vm.md:13-18`
- `specs/vm.md:179-188`

**Status:** pending

## STORY-0071

**Epic:** EPIC-022 — Indexed RPN Execution
**Title:** Support literal value loading instructions

**As a** bootstrap pipeline
**I want** literal instructions (LoadNull, LoadBool, LoadInt, LoadFloat, LoadString, LoadSymbol) to store their values at values[ip]
**So that** compiled constants are available as operands for subsequent instructions

**Acceptance criteria:**
- AC-1: LoadNull stores Value::Null at values[ip] · impact:`local` · seam:`unit`
- AC-2: LoadBool(b) stores Value::Bool(b) at values[ip] · impact:`local` · seam:`unit`
- AC-3: LoadInt(n) stores Value::Int(n) at values[ip] · impact:`local` · seam:`unit`
- AC-4: LoadFloat(f) stores Value::Float(f) at values[ip] · impact:`local` · seam:`unit`
- AC-5: LoadString(s) stores Value::String(s) at values[ip] · impact:`local` · seam:`unit`
- AC-6: LoadSymbol(s) stores Value::Symbol(s) at values[ip] · impact:`local` · seam:`unit`

**Sources:**
- `specs/vm.md:57-64`

**Status:** pending

## STORY-0072

**Epic:** EPIC-022 — Indexed RPN Execution
**Title:** Execute comparison instructions

**As a** bootstrap pipeline
**I want** comparison instructions (Eq, NotEq, Lt, Gt, LtEq, GtEq) to produce boolean results from indexed operands
**So that** conditional logic in compiled code works correctly

**Acceptance criteria:**
- AC-1: Eq produces true when values[lhs] equals values[rhs], false otherwise · impact:`cross-surface` · seam:`unit`
- AC-2: Lt produces true when values[lhs] is less than values[rhs] · impact:`local` · seam:`unit`
- AC-3: NotEq, Gt, LtEq, GtEq produce correct boolean results for their respective comparisons · impact:`local` · seam:`unit`

**Sources:**
- `specs/vm.md:87-94`

**Status:** pending

## STORY-0073

**Epic:** EPIC-022 — Indexed RPN Execution
**Title:** Execute pipe operator as function application

**As a** bootstrap pipeline
**I want** Pipe instruction to apply a function to an argument (x |> f becomes f(x))
**So that** pipeline-style code compiles and runs correctly

**Acceptance criteria:**
- AC-1: Pipe { arg, func } evaluates to func(arg), calling the function at values[func] with values[arg] as the single argument · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0060`

**Sources:**
- `specs/vm.md:136`
- `specs/vm.md:257`

**Status:** pending
