@a study specs/README.md
Remember:
@c study the jj-workflow skill - it will help you with commiting code and managing tasks.
Rules:

0. Always follow the jj-workflow for managing tasks and commits.
1. Use `jj issue ready` to find the highest priority task.
2. Use `jj issue show <id>` to get the task details. The description IS your research — it contains the key files, code examples, and context you need. Do NOT re-read files already quoted in the issue unless you need surrounding context for an edit.
3. Go directly to implementation after reading the issue. Stop as soon as you've completed your selected task.
4. If you discover something that needs to be fixed in the process of completing your task, document it in the specs/ directory.
5. Fixes should not be considered done until they are verified and committed to the repository.
6. Use subagents for research that goes beyond what the issue description provides. Context is precious.
7. Update AGENTS.md as necessary to include build instructions. Avoid polluting it with design decisions.
8. Anything that looks like it is actually a design decision, rather than an implementation plan, should go in docs/
9. Make sure all specs are clear, concise, and unambiguous. If they're longer than about 200 lines, break the spec into a top level file, a directory, and subspecs within that directory.
10. Use TDD. Make sure to use DRY, KISS, and YAGNI principles.
11. Always filter cargo output — see AGENTS.md "Operating Instructions" section.
12. cargo test should always pass before you commit. After entering TDD green mode, don't fix failing tests by changing the test itself.
13. cargo clippy must always pass before you commit - all suggestions should be applied. If you need to add an allow, add it to the top of the file with a comment explaining why.
14. There MUST be no warnings while building.
15. Filter ALL cargo output (build, test, check, clippy) — not just tests. See AGENTS.md for the filter commands.
16. Limit issue selection: `jj issue ready | head -5`
17. For external crate APIs, use context7 (resolve-library-id + query-docs) or fetch docs.rs. Never grep ~/.cargo/registry.
18. Read files once, generously. Don't re-read narrow windows of the same file.
19. Context is precious. One filtered build command, one docs lookup, one file read.
20. Never declare done while the build is broken. The last cargo build/test must show zero errors.
21. If you hit the same error 3 times: write a spec with what you tried and what failed, comment the spec path on the issue (`jj issue comment <id> "Blocked: see specs/<path>"`), then stop. Do not continue.
