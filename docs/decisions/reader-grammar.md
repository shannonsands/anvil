# Reader Grammar

Canonical planning note:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/reader-grammar.md`

Implementation-facing decision:

- Anvil starts with a Lisp reader, not a full grammar-heavy parser.
- The first reader surface is intentionally Clojure-friendly: lists use `()`,
  vectors use `[]`, and maps use `{}`.
- Whitespace separates forms. Commas are whitespace so vector/map-heavy code can
  breathe when useful.
- `;` starts a line comment.
- Strings use double quotes and support `\n`, `\r`, `\t`, `\"`, and `\\`.
- Quote sugar uses `'datum` and reads as a quote datum. AST lowering preserves
  it as quote, and the bootstrap VM compiles quote to immutable data; later
  macro work can still decide whether and how to lower quote into core forms.
- Atoms are interpreted by the reader as `nil`, booleans, keywords, signed
  `i64` integers, `f64` decimal/exponent literals, or symbols.
- Maps require an even number of forms and preserve key/value order in the datum
  representation.
- Every datum and reader diagnostic carries a source span.

Deliberately deferred:

- BigInt, Ratio, complex, exactness syntax, and dtype suffixes. Numeric
  semantics are locked, but literal sugar should be added with focused specs.
- Reader macros beyond quote.
- Namespaces and module paths.
- Metadata, dispatch forms, regex literals, tagged literals, and set literals.
- Full AST validation, macro expansion, and module-aware evaluation.

The reader-backed REPL is the first consumer of this grammar. It should remain
honest that evaluation is not implemented yet.
