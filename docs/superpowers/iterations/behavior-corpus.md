# Behavior Corpus

| Scenario ID | Title | Proof seam | Run cadence | Command | Owning stories |
|---|---|---|---|---|---|
| SCENARIO-0001 |  | integration | iteration | TBD | STORY-0001, STORY-0003 |
| SCENARIO-0002 |  | app-level | iteration | TBD | STORY-0001, STORY-0002 |
| SCENARIO-0003 |  | integration | iteration | TBD | STORY-0005, STORY-0006 |
| SCENARIO-0004 |  | app-level | iteration | TBD | STORY-0017, STORY-0013, STORY-0014... |
| SCENARIO-0005 |  | app-level | iteration | TBD | STORY-0017 |
| SCENARIO-0006 |  | app-level | iteration | TBD | STORY-0017 |
| SCENARIO-0007 |  | process-level | iteration | TBD | STORY-0022 |
| SCENARIO-0008 |  | process-level | iteration | TBD | STORY-0023, STORY-0018 |
| SCENARIO-0009 |  | app-level | iteration | TBD | STORY-0018 |
| SCENARIO-0010 |  | integration | iteration | TBD | STORY-0029, STORY-0030 |
| SCENARIO-0011 |  | integration | iteration | TBD | STORY-0031 |
| SCENARIO-0012 |  | e2e | sentinel | TBD | STORY-0033, STORY-0034, STORY-0036 |
| SCENARIO-0013 |  | e2e | sentinel | TBD | STORY-0037 |
| SCENARIO-0014 |  | integration | iteration | TBD | STORY-0024 |
| SCENARIO-0015 |  | app-level | iteration | TBD | STORY-0038 |
| SCENARIO-0016 | Parity contract: FMPL pipeline vs Rust compiler | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity` | STORY-0007, STORY-0008 |
| SCENARIO-0017 |  | integration | iteration | TBD | STORY-0013 |
| SCENARIO-0018 |  | integration | iteration | TBD | STORY-0014 |
| SCENARIO-0019 |  | integration | iteration | TBD | STORY-0020 |
| SCENARIO-0020 |  | e2e | sentinel | TBD | STORY-0025 |
| SCENARIO-0021 |  | e2e | sentinel | TBD | STORY-0026 |
| SCENARIO-0022 |  | integration | iteration | TBD | STORY-0040 |
| SCENARIO-0023 |  | integration | iteration | TBD | STORY-0042 |
| SCENARIO-0024 |  | integration | iteration | TBD | STORY-0043 |
| SCENARIO-0025 |  | integration | iteration | TBD | STORY-0043 |
| SCENARIO-0026 |  | integration | iteration | TBD | STORY-0044 |
| SCENARIO-0027 |  | integration | iteration | TBD | STORY-0045 |
| SCENARIO-0028 |  | integration | iteration | TBD | STORY-0046 |
| SCENARIO-0029 |  | integration | iteration | TBD | STORY-0047 |
| SCENARIO-0030 | Full pipeline integer parity | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity parity_integer` | STORY-0048 |
| SCENARIO-0031 | Full pipeline arithmetic parity | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity parity_arithmetic` | STORY-0048 |
| SCENARIO-0032 | Full pipeline string parity | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity parity_string` | STORY-0048 |
| SCENARIO-0033 | Full pipeline let binding parity | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity parity_let_binding` | STORY-0048 |
| SCENARIO-0034 | Full pipeline if expression parity | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity parity_if_expr` | STORY-0048 |
| SCENARIO-0035 | Full pipeline lambda parity | integration | iteration | BLOCKED:grammar-engine-star-in-tagmatch | STORY-0048 |
| SCENARIO-0036 | Full pipeline list parity | integration | iteration | BLOCKED:grammar-engine-star-in-tagmatch | STORY-0048 |
| SCENARIO-0037 | Full pipeline map parity | integration | iteration | BLOCKED:grammar-engine-star-in-tagmatch | STORY-0048 |
| SCENARIO-0038 | Pipeline setup loads prelude and ast_to_ir | integration | sentinel | `cargo test -p fmpl-core --test ast_to_ir_parity parity_symbol` | STORY-0048 |
| SCENARIO-0039 |  | integration | iteration | TBD | STORY-0057, STORY-0054, STORY-0053 |
| SCENARIO-0040 |  | integration | iteration | TBD | STORY-0051 |
| SCENARIO-0041 |  | integration | iteration | TBD | STORY-0051 |
| SCENARIO-0042 |  | integration | iteration | TBD | STORY-0053 |
| SCENARIO-0043 |  | integration | iteration | TBD | STORY-0053 |
| SCENARIO-0044 |  | unit | iteration | TBD | STORY-0054, STORY-0055 |
| SCENARIO-0045 |  | integration | iteration | TBD | STORY-0055, STORY-0061 |
| SCENARIO-0046 |  | integration | iteration | TBD | STORY-0062 |
| SCENARIO-0047 |  | integration | iteration | TBD | STORY-0064 |
| SCENARIO-0048 |  | integration | iteration | TBD | STORY-0066 |
| SCENARIO-0049 |  | integration | iteration | TBD | STORY-0052 |
| SCENARIO-0050 |  | integration | iteration | TBD | STORY-0050 |
| SCENARIO-0051 |  | unit | iteration | TBD | STORY-0059 |
| SCENARIO-0052 |  | integration | iteration | TBD | STORY-0067 |
| SCENARIO-0053 |  | integration | iteration | TBD | STORY-0070 |
| SCENARIO-0054 |  | integration | iteration | TBD | STORY-0074 |
| SCENARIO-0055 |  | integration | iteration | TBD | STORY-0076 |
| SCENARIO-0056 |  | integration | iteration | TBD | STORY-0079 |
| SCENARIO-0057 |  | integration | iteration | TBD | STORY-0086 |
| SCENARIO-0058 |  | integration | iteration | TBD | STORY-0082 |
| SCENARIO-0059 |  | integration | iteration | TBD | STORY-0077 |
| SCENARIO-0060 |  | integration | iteration | TBD | STORY-0073 |
| SCENARIO-0061 |  | integration | iteration | TBD | STORY-0085 |
| SCENARIO-0062 |  | integration | iteration | TBD | STORY-0075 |
| SCENARIO-0063 |  | integration | iteration | TBD | STORY-0087 |
| SCENARIO-0064 |  | unit | iteration | TBD | STORY-0084 |
| SCENARIO-0065 |  | integration | iteration | TBD | STORY-0089, STORY-0092, STORY-0093... |
| SCENARIO-0066 |  | unit | iteration | TBD | STORY-0095 |
| SCENARIO-0067 |  | integration | iteration | TBD | STORY-0095, STORY-0097, STORY-0096 |
| SCENARIO-0068 |  | integration | iteration | TBD | STORY-0096 |
| SCENARIO-0069 |  | integration | iteration | TBD | STORY-0069, STORY-0097 |
| SCENARIO-0070 |  | integration | iteration | TBD | STORY-0090 |
| SCENARIO-0071 |  | integration | iteration | TBD | STORY-0089 |
| SCENARIO-0072 |  | integration | iteration | TBD |  |
| SCENARIO-0073 |  | integration | iteration | TBD |  |
| SCENARIO-0074 |  | app-level | iteration | TBD | STORY-0038 |
| SCENARIO-0075 |  | integration | iteration | TBD | STORY-0001 |
| SCENARIO-0076 |  | integration | iteration | TBD | STORY-0001 |
| SCENARIO-0077 |  | app-level | iteration | TBD | STORY-0038 |
| SCENARIO-0103 | Full parity corpus passes with optimizer enabled | integration | sentinel | TBD | STORY-0010 |
| SCENARIO-0099 | Loader skips records with incompatible VM version | integration | iteration | TBD | STORY-0099 |
| SCENARIO-0100 | Bytecode persists with content-addressed source reference | integration | iteration | TBD | STORY-0100 |
| SCENARIO-0101 | Sourceless artifact gets synthesized constructor expression | integration | iteration | TBD | STORY-0100 |
| SCENARIO-0102 | Loader recovers from incompatible payload via source recompilation | integration | iteration | TBD | STORY-0100 |
