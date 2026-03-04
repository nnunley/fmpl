# EPIC-031 — Core Evaluation Pipeline

**Summary:** Core Evaluation Pipeline
**Stories:** STORY-0089, STORY-0090, STORY-0091, STORY-0092, STORY-0093, STORY-0094
**Primary sources:** `specs/fmpl-core.md`
**Status:** 0/6 done

## STORY-0089

**Epic:** EPIC-031 — Core Evaluation Pipeline
**Title:** Evaluate FMPL source code end-to-end

**As a** FMPL runtime consumer
**I want** to evaluate FMPL source strings through eval(vm, source)
**So that** source code is lexed, parsed, compiled, and executed in a single call

**Acceptance criteria:**
- AC-1: eval(vm, '1 + 2 * 3') returns Int(7) respecting operator precedence · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0065`
- AC-2: eval requires a Vm initialized with an ObjectDb · impact:`local` · seam:`unit` · scenario:`SCENARIO-0065`
- AC-3: eval returns Result<Value> with meaningful error on parse or runtime failure · impact:`local` · seam:`integration` · scenario:`SCENARIO-0065`

**Sources:**
- `specs/fmpl-core.md:74-88`
- `specs/fmpl-core.md:158-172`

**Status:** pending

## STORY-0090

**Epic:** EPIC-031 — Core Evaluation Pipeline
**Title:** Expose complete public API surface

**As a** downstream crate consumer
**I want** fmpl-core to export Expr, CompiledCode, Compiler, Grammar, GrammarRegistry, Pattern, Rule, Lexer, Token, Object, ObjectDb, ObjectId, Parser, Value, and Vm
**So that** all pipeline stages are accessible for embedding and extension

**Acceptance criteria:**
- AC-1: All types listed in the public API section (lines 76-87) are re-exported from fmpl_core · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0070`
- AC-2: The eval function is publicly accessible as fmpl_core::eval · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0070`

**Sources:**
- `specs/fmpl-core.md:74-88`

**Status:** pending

## STORY-0091

**Epic:** EPIC-031 — Core Evaluation Pipeline
**Title:** Compile AST to indexed RPN bytecode

**As a** FMPL compiler
**I want** the Compiler to transform AST Expr nodes into CompiledCode (indexed RPN bytecode)
**So that** parsed programs can be executed by the VM

**Acceptance criteria:**
- AC-1: Compiler accepts AST Expr and produces CompiledCode · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0065`
- AC-2: CompiledCode is representable as a Code value for first-class code objects · impact:`local` · seam:`unit` · scenario:`SCENARIO-0065`

**Sources:**
- `specs/fmpl-core.md:15-17`
- `specs/fmpl-core.md:82-83`

**Status:** pending

## STORY-0092

**Epic:** EPIC-031 — Core Evaluation Pipeline
**Title:** Tokenize source with logos-based lexer

**As a** FMPL parser
**I want** the Lexer to tokenize source strings into Token sequences using logos
**So that** the parser receives a well-defined token stream

**Acceptance criteria:**
- AC-1: Lexer produces Token values from source strings · impact:`journey` · seam:`unit` · scenario:`SCENARIO-0065`
- AC-2: Lexer is publicly exported as fmpl_core::Lexer with Token type · impact:`local` · seam:`unit` · scenario:`SCENARIO-0065`

**Sources:**
- `specs/fmpl-core.md:14`
- `specs/fmpl-core.md:84`

**Status:** pending

## STORY-0093

**Epic:** EPIC-031 — Core Evaluation Pipeline
**Title:** Parse token stream into AST

**As a** FMPL compiler
**I want** the Parser to produce AST Expr nodes from tokenized source via recursive descent
**So that** source programs have a structured representation for compilation

**Acceptance criteria:**
- AC-1: Parser produces Expr AST nodes from source · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0065`
- AC-2: Parser supports generated parser mode in addition to built-in recursive descent · impact:`local` · seam:`integration` · scenario:`SCENARIO-0065`

**Sources:**
- `specs/fmpl-core.md:15`
- `specs/fmpl-core.md:29`

**Status:** pending

## STORY-0094

**Epic:** EPIC-031 — Core Evaluation Pipeline
**Title:** Execute indexed RPN bytecode in VM

**As a** FMPL runtime
**I want** the VM to execute CompiledCode and produce runtime Values with async support
**So that** compiled programs produce observable results including async operations

**Acceptance criteria:**
- AC-1: VM executes CompiledCode and returns Value results · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0065`
- AC-2: VM supports async operations via the tokio runtime · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0065`

**Sources:**
- `specs/fmpl-core.md:16-17`
- `specs/fmpl-core.md:33-34`

**Status:** pending
