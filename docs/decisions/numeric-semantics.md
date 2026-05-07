# Numeric Semantics

The canonical planning note is:

`/Users/shannon/Workspace/Workspace/Obsidian/Global/Projects/anvil-language-runtime/numeric-semantics.md`

Implementation-facing decision:

- Anvil keeps an exact scalar core without adopting the full Scheme numeric
  tower.
- Integer literals are exact. Exact integer division returns an exact integer
  when divisible, otherwise `Ratio`.
- Decimal literals default to `Float64`; `Float32` is explicit or
  context-driven.
- Exact scalar arithmetic does not silently wrap; fixed-width and tensor
  integer overflow requires checked or explicit wrapping/saturating/truncating
  behavior.
- Floats follow IEEE semantics, including `NaN`, infinities, and signed zero.
- Numeric equality leaves `NaN` unequal; complex equality is component-wise;
  approximate equality is explicit and never used for hard logic, hard types,
  map keys, or authority.
- `Prob` is a checked constrained type in `[0, 1]`, not a raw float alias.
- Tensor dtypes are strict: tensor-to-tensor dtype conversion is explicit;
  scalar literals may adapt to tensor context when representable.
- Reserve first-class vector/tensor operations including elementwise arithmetic,
  `mapv`, reductions, `dot`, `matmul`, shape transforms, `rearrange`, and later
  `einsum`.
- VSA/FHRR phase-vector operations depend on complex dtypes and dimension
  tracking.

Open implementation dependency: literal spelling and cast/operator names should
be finalized with the reader/syntax decision.
