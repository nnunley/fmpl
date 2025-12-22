# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

FMPL ("of Accardi") is a prototype-based object-oriented programming language developed at the Experimental Computing Facility (XCF) of UC Berkeley in 1992. This repository contains an EBNF grammar specification (`fmpl.ebnf`).

**Key characteristics:**
- Prototype-based OOP (not class-based)
- Lambda calculus constructs
- Event-driven with I/O stream events (Unix domain and internet sockets)
- Pretty-printing of internal code representation

Original release: Jon Blow (blojo@xcf.berkeley.edu), June 2, 1992

## Grammar Structure

The grammar defines a language with:
- **Expressions** (`<exp>`): Core construct supporting arithmetic, logical, comparison, and composition operators
- **Control flow**: if/then/else, while/do, do/while, return
- **Functions**: Named functions, lambdas (`\x expr`), function calls with parameter lists
- **Data structures**: Lists `[]`, hash tables `htable()`, objects with tagged properties
- **Bindings**: let-bindings, object property bindings with public/private modifiers
- **Object system**: Object definitions with inheritance (`<olist>`) and sparse structures

## Key Tokens

Terminal tokens: `TOKEN_OFFSET`, `FMPL_GC_COPIED`, `FLOAT`, `OBJ_TAG`, `TAG`, `STRING`, `SYMBOL`, `FN_TAG`

Special self-evaluating forms: `T_ARGS`, `T_SELF`, `T_PARENT`, `T_CALLER`, `T_USER`, `T_NULL`, `T_FUN`

## Grammar Conventions

- Optional elements use `[ ]` brackets
- Alternatives separated by newlines (not `|`)
- `<error>` productions handle malformed input
- Optional separators: commas between list items are often optional
- Optional semicolons between statements (`<optsemi>`)

## If Implementing This Language

1. Choose a parser generator (ANTLR, yacc/bison, or hand-written recursive descent)
2. Implement lexer for terminal tokens (TAG, FLOAT, STRING, etc.)
3. Handle operator precedence (not explicitly defined in grammar)
4. Implement semantic analysis for scoping and type checking
5. Build interpreter or compiler backend
