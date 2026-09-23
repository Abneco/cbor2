# Derive review fixes — 2026-09-23

## Correctness

- Infer delegating impl bounds from participating fields and variants. Honor
  field/variant `bound`, custom codecs, skipped fields, associated types and
  the `Default` requirement for skipped/defaulted generic fields.
- Constrain the deserializer lifetime only by actual borrowed fields. Owned
  `Cow` and skipped lifetime markers can implement `DeserializeOwned` and use
  `from_reader`; implicit string/byte borrowing and explicit `borrow` remain
  supported.
- Reject inconsistent integer/text mappings of the same serde name across
  enum variants, including explicit and rule-based renames.
- Reject untagged variants in integer-key enums and serde struct tags combined
  with CBOR metadata. These previously compiled but could change protocol
  fields or fail to decode their own output.
- Preserve the original type name for plain serde struct tags, rather than
  exposing the internal shadow name.
- Ignore fully skipped flatten fields when choosing the binary adapter.

Nested struct flattening continues to use serde field names. It does not
inherit the inner type's CBOR keys, tag or array shape. This boundary is now
documented in both derive READMEs and covered by a wire/Value regression test.
Use outer declared integer-key fields plus an extension map for protocol
labels; this change does not introduce a nested metadata protocol.

## Implementation and performance

Container, variant and field serde attributes are scanned once. Shadow
preparation and impl-bound construction have separate functions/modules;
macro unit tests live in their own module. Lifetime renaming traverses the
syn AST instead of converting predicates through token streams. The unused
`syn/full` feature is removed.

Integer-key encoding now searches directly for the field name and checks
entry boundaries. It parses only candidate integer values, retaining the
first-valid-entry behavior for hand-written markers. This avoids a stateful
cursor or cache and speeds up both streaming and Value serializers.

A local release microbenchmark on arm64, rustc 1.98.1, reused an output Vec
and encoded equal CBOR outputs in three rounds of 200,000 items. Median times:

| Integer-key fields | Before | After |
| --- | ---: | ---: |
| 4 × u64 | 304 ns | 133 ns |
| 16 × u64 | 2,824 ns | 698 ns |

These measurements isolate small-record key overhead, not application
throughput. They are not Criterion confidence intervals. The equivalent
workloads are now available in the standalone benchmark crate, alongside
empty-extension flatten and hand-written integer-map controls:

```sh
cargo bench --manifest-path cbor2-bench/Cargo.toml --bench derive
```

## Verification

Passed:

```sh
cargo fmt --all --check
cargo fmt --manifest-path tests/derive-fixtures/Cargo.toml --check
cargo fmt --manifest-path cbor2-bench/Cargo.toml --check
cargo test --workspace --all-targets --all-features
cargo test --workspace --all-features --doc
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p cbor2 --no-default-features --lib
cargo test -p cbor2 --no-default-features --features alloc --lib
cargo +1.89.0 test -p cbor2 --features derive --test it --target-dir target/derive-msrv
cargo check --manifest-path tests/derive-fixtures/Cargo.toml --lib --no-default-features --target thumbv7em-none-eabihf
git diff --check
```

The consumer tests include executable generic/borrowing cases, compile-fail
attribute combinations and a no_std + alloc library. CI also compiles that
consumer on the bare-metal target. The new Criterion target was compiled and
run with a short 10-sample smoke configuration.
