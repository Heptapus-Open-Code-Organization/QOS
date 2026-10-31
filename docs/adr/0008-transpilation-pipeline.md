# ADR-0008: Transpilation pipeline

- **Status:** Accepted
- **Date:** 2026-06-25
- **Deciders:** QOS team
- **Related ADRs:** ADR-0005 (Circuit IR), ADR-0006 (topology/layout), ADR-0007 (calibration cost)

## Context

A logical circuit almost never runs unchanged on real hardware: the device has a limited
**native gate set**, a fixed **connectivity**, and varying **qubit quality**. The circuit
must be rewritten to those constraints. The kernel has a `CircuitTranspiler` stub but no real
passes. This is mandatory infrastructure for any hardware (and useful even for the simulator
as a no-op/identity to keep the path live).

## Decision

Define a **staged, pluggable transpilation pipeline** in `qos-core` that maps a logical
`Circuit` (ADR-0005) to a backend-runnable `Circuit`:

1. **Optimize (pre-layout):** trivial simplifications (cancel adjacent inverses, merge
   rotations). Controlled by `optimization_level` (0–3, already in `qos-abi::JobOptions`).
2. **Layout:** choose a logical→physical qubit `Layout` (ADR-0006), using calibration quality
   (ADR-0007) when available.
3. **Routing:** insert SWAPs so every two-qubit gate acts on physically-connected qubits per
   the `Topology`.
4. **Native decomposition:** rewrite gates into the backend's native gate set
   (`capabilities().native_gates` from ADR-0004).
5. **Optimize (post):** clean up redundant gates introduced by routing/decomposition.

Each stage is a pass with a common interface; passes can be skipped per backend. For the
**simulator** (all-to-all, universal gate set) the pipeline is effectively identity, but runs
the same code so it is continuously exercised.

## Rationale

- Staged passes are the standard, well-understood structure (mirrors Qiskit/t|ket⟩) and keep
  each transformation testable in isolation.
- A pluggable pipeline lets a backend declare which stages it needs, so the simulator stays
  cheap while hardware gets the full treatment.

## Consequences

### Positive

- Hardware circuits become runnable; the simulator path validates the pipeline plumbing.
- `optimization_level` and `native_gates` gain concrete meaning.

### Negative / Trade-offs

- Routing/layout are genuinely hard problems; the first implementations will be simple
  (greedy SWAP routing, first-fit layout) and improved later.

### Neutral / Follow-ups

- Pass quality metrics (added gate count, estimated fidelity from calibration) are reported in
  job metadata for observability.

## Implementation increment (2026-10-31)

The portable `qos-core::route_circuit` now implements the first routing stage for CX, CZ and
SWAP over validated, undirected connectivity graphs. It uses shortest paths within the active
qubit register, inserts SWAPs to bring the first operand adjacent to the second, then reverses
those routing SWAPs. Restoring the identity layout preserves subsequent gates, classical
measurement destinations, and output wire order without a result-remapping contract. The
returned `RoutingResult` exposes `inserted_swaps`; the input is unchanged and the output starts
at instruction zero.

`QuantumBackend::prepare` routes and then validates the prepared circuit against backend
capabilities. The portable local simulator's submission path calls it, exercising the
all-to-all identity case. Validation rejects invalid register indices, repeated gate operands,
non-finite angles, unsupported native gates, unsupported operations after measurement, and
invalid topology graphs before execution. Direct `validate` checks an already prepared circuit
and rejects nonlocal gates. Backend implementations must call `prepare` from their submission
path if they accept logical circuits.

This is a conservative first pass: it adds more SWAPs than a persistent layout would, does not
use spare physical wires, and requires native SWAP support whenever routing inserts SWAPs.
Directed couplings and native-gate decomposition are not implemented; CCX requires pairwise
connectivity. Calibration-aware layout, optimization stages, and kernel IDE/Lab integration
remain follow-ups. The kernel still has its separate simulator/toolchain; this increment does
not claim a remote provider or a change to the desktop execution path.

Verification: `cargo test -p qos-core --features std` covers statevector equivalence on every
four-qubit basis input for CX/CZ/SWAP over linear and grid graphs (including complex phases
and subsequent gates), all-to-all identity, measurement-destination preservation in the IR,
capability failures, malformed/disconnected graphs, and invalid-input rejection at submission.
