//! Connectivity routing for the portable QHAL (ADR-0008).
//!
//! The initial router uses the identity layout and restores it after each two-qubit gate.
//! This costs extra SWAPs, but preserves wire order, measurement destinations and subsequent
//! gates without a hidden final permutation. Only the circuit's active qubits are used.

use alloc::format;

use crate::{BackendCapabilities, BackendError, Circuit, Gate, GateType, Topology};

#[derive(Clone, Debug)]
pub struct RoutingResult {
    pub circuit: Circuit,
    /// Number of routing SWAPs, excluding SWAPs already in the input circuit.
    pub inserted_swaps: usize,
}

fn unsupported(message: &str) -> BackendError {
    BackendError::UnsupportedCircuit(message.into())
}

/// Validate IR before indexing qubit/classical registers or applying gates.
pub(crate) fn validate_circuit(circuit: &Circuit) -> Result<(), BackendError> {
    for gate in &circuit.gates {
        let qubit = |q: usize| {
            if q < circuit.n_qubits {
                Ok(())
            } else {
                Err(unsupported("qubit index out of range"))
            }
        };
        let pair = |a, b| {
            qubit(a)?;
            qubit(b)?;
            if a != b {
                Ok(())
            } else {
                Err(unsupported("gate operands must be distinct"))
            }
        };
        match gate {
            Gate::Cx(a, b) | Gate::Cz(a, b) | Gate::Swap(a, b) => pair(*a, *b)?,
            Gate::Ccx(a, b, c) => {
                pair(*a, *b)?;
                pair(*a, *c)?;
                pair(*b, *c)?;
            }
            Gate::Barrier(qubits) => {
                for &q in qubits {
                    qubit(q)?;
                }
            }
            Gate::Measure(q, c) => {
                qubit(*q)?;
                if *c >= circuit.n_cbits {
                    return Err(unsupported("classical bit index out of range"));
                }
            }
            Gate::Rx(q, angle) | Gate::Ry(q, angle) | Gate::Rz(q, angle) => {
                qubit(*q)?;
                if !angle.is_finite() {
                    return Err(unsupported("gate angles must be finite"));
                }
            }
            Gate::U3(q, theta, phi, lambda) => {
                qubit(*q)?;
                if !theta.is_finite() || !phi.is_finite() || !lambda.is_finite() {
                    return Err(unsupported("gate angles must be finite"));
                }
            }
            Gate::H(q)
            | Gate::X(q)
            | Gate::Y(q)
            | Gate::Z(q)
            | Gate::S(q)
            | Gate::Sdg(q)
            | Gate::T(q)
            | Gate::Tdg(q)
            | Gate::Id(q)
            | Gate::Reset(q) => qubit(*q)?,
        }
    }
    Ok(())
}

pub(crate) fn gate_type(gate: &Gate) -> GateType {
    match gate {
        Gate::H(_) => GateType::H,
        Gate::X(_) => GateType::X,
        Gate::Y(_) => GateType::Y,
        Gate::Z(_) => GateType::Z,
        Gate::S(_) => GateType::S,
        Gate::Sdg(_) => GateType::Sdg,
        Gate::T(_) => GateType::T,
        Gate::Tdg(_) => GateType::Tdg,
        Gate::Rx(..) => GateType::Rx,
        Gate::Ry(..) => GateType::Ry,
        Gate::Rz(..) => GateType::Rz,
        Gate::U3(..) => GateType::U3,
        Gate::Id(_) => GateType::Id,
        Gate::Cx(..) => GateType::Cx,
        Gate::Cz(..) => GateType::Cz,
        Gate::Swap(..) => GateType::Swap,
        Gate::Ccx(..) => GateType::Ccx,
        Gate::Measure(..) => GateType::Measure,
        Gate::Reset(_) => GateType::Reset,
        Gate::Barrier(_) => GateType::Barrier,
    }
}

pub(crate) fn validate_capabilities(
    circuit: &Circuit,
    caps: &BackendCapabilities,
) -> Result<(), BackendError> {
    validate_circuit(circuit)?;
    let mut measured = false;
    for gate in &circuit.gates {
        if !caps.native_gates.contains(&gate_type(gate)) {
            return Err(BackendError::UnsupportedCircuit(format!(
                "unsupported native gate: {:?}",
                gate_type(gate)
            )));
        }
        match gate {
            Gate::Measure(..) => measured = true,
            Gate::Barrier(_) => {}
            _ if measured && !caps.mid_circuit_measurement => {
                return Err(unsupported(
                    "backend does not support operations after measurement",
                ));
            }
            _ => {}
        }
        if let Some(topo) = &caps.topology {
            let connected = |a, b| {
                if topo.is_connected(a, b) && topo.is_connected(b, a) {
                    Ok(())
                } else {
                    Err(unsupported("gate requires connectivity routing"))
                }
            };
            match gate {
                Gate::Cx(a, b) | Gate::Cz(a, b) | Gate::Swap(a, b) => connected(*a, *b)?,
                Gate::Ccx(a, b, c) => {
                    connected(*a, *b)?;
                    connected(*a, *c)?;
                    connected(*b, *c)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Route CX/CZ/SWAP gates over a validated, undirected topology.
///
/// Each nonlocal gate moves its first operand next to its second along a shortest path,
/// applies the gate, then reverses the routing SWAPs. CCX must already have pairwise
/// connectivity; decomposition, directed couplings and use of spare qubits are deferred.
/// The returned circuit starts at pc=0; the input is never modified.
///
/// ```
/// use qos_core::{route_circuit, Circuit, Topology};
/// let mut circuit = Circuit::new(4, 4);
/// circuit.h(0).cx(0, 3);
/// let routed = route_circuit(&circuit, &Topology::linear(4)).unwrap();
/// assert_eq!(routed.inserted_swaps, 4);
/// assert_eq!(routed.circuit.len(), 6);
/// ```
pub fn route_circuit(
    circuit: &Circuit,
    topology: &Topology,
) -> Result<RoutingResult, BackendError> {
    validate_circuit(circuit)?;
    topology.validate()?;
    if circuit.n_qubits > topology.n_qubits {
        return Err(unsupported("circuit exceeds backend topology size"));
    }
    // Exclude spare physical wires: using them would change the circuit/output width.
    let active = Topology {
        n_qubits: circuit.n_qubits,
        connections: topology.connections[..circuit.n_qubits]
            .iter()
            .map(|adj| {
                adj.iter()
                    .copied()
                    .filter(|&q| q < circuit.n_qubits)
                    .collect()
            })
            .collect(),
    };
    let mut routed = Circuit::new(circuit.n_qubits, circuit.n_cbits);
    let mut inserted_swaps = 0;
    for gate in &circuit.gates {
        let (a, b) = match gate {
            Gate::Cx(a, b) | Gate::Cz(a, b) | Gate::Swap(a, b) => (*a, *b),
            Gate::Ccx(a, b, c) => {
                if !active.is_connected(*a, *b)
                    || !active.is_connected(*a, *c)
                    || !active.is_connected(*b, *c)
                {
                    return Err(unsupported(
                        "CCX requires pairwise connectivity; decomposition is not implemented",
                    ));
                }
                routed.add(gate.clone());
                continue;
            }
            _ => {
                routed.add(gate.clone());
                continue;
            }
        };
        // Keep already-local gates (especially the all-to-all simulator path) cheap.
        if active.is_connected(a, b) {
            routed.add(gate.clone());
            continue;
        }
        let path = active
            .shortest_path(a, b)
            .ok_or_else(|| unsupported("no connectivity path between gate operands"))?;
        let swaps = &path[..path.len() - 1];
        for edge in swaps.windows(2) {
            routed.add(Gate::Swap(edge[0], edge[1]));
        }
        let adjacent = path[path.len() - 2];
        routed.add(match gate {
            Gate::Cx(..) => Gate::Cx(adjacent, b),
            Gate::Cz(..) => Gate::Cz(adjacent, b),
            _ => Gate::Swap(adjacent, b),
        });
        for edge in swaps.windows(2).rev() {
            routed.add(Gate::Swap(edge[0], edge[1]));
        }
        inserted_swaps += 2 * (path.len() - 2);
    }
    Ok(RoutingResult {
        circuit: routed,
        inserted_swaps,
    })
}
