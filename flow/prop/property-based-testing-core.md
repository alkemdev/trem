# Property-based tests for core invariants

## What

Add `proptest` or `quickcheck` (workspace dev-deps) for `Rational` arithmetic, `Tree` normalization, and grid rendering: e.g. folding never inverts order, spans stay within parents.

## Why

Exact arithmetic and recursive trees are easy to get subtly wrong; examples alone miss edge cases.

## Notes

- Keep run time bounded; use shrinking-friendly generators for `Rational`.
- Document which properties are "soft" (float boundary) vs hard invariants.
