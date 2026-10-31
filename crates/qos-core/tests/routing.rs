#![cfg(feature = "std")]

use qos_core::{
    route_circuit, BackendCapabilities, BackendError, BackendJobId, BackendResult, BackendStatus,
    Circuit, Gate, GateType, JobState, LocalSimulatorBackend, QuantumBackend, QuantumState,
    Topology,
};

struct Device(BackendCapabilities);

impl Device {
    fn linear(n: usize) -> Self {
        Self(BackendCapabilities {
            max_qubits: n,
            native_gates: GateType::universal_set(),
            topology: Some(Topology::linear(n)),
            mid_circuit_measurement: true,
        })
    }
}

impl QuantumBackend for Device {
    fn name(&self) -> &str {
        "test-device"
    }
    fn capabilities(&self) -> &BackendCapabilities {
        &self.0
    }
    fn status(&self) -> BackendStatus {
        BackendStatus::Available
    }
    fn submit(&mut self, _: &Circuit, _: u64) -> Result<BackendJobId, BackendError> {
        Err(BackendError::NotAvailable)
    }
    fn poll(&self, _: BackendJobId) -> Option<JobState> {
        None
    }
    fn result(&self, _: BackendJobId) -> Option<BackendResult> {
        None
    }
}

fn assert_same_state(input: &Circuit, output: &Circuit, basis: usize) {
    let mut expected = QuantumState::new(input.n_qubits);
    expected.amplitudes.fill(qos_core::Complex::ZERO);
    expected.amplitudes[basis] = qos_core::Complex::ONE;
    let mut actual = expected.clone();
    let mut input = input.clone();
    input.reset_pc();
    input.run(&mut expected);
    let mut output = output.clone();
    output.reset_pc();
    output.run(&mut actual);
    for (a, b) in expected.amplitudes.iter().zip(&actual.amplitudes) {
        assert!(
            (a.re - b.re).abs() < 1e-10 && (a.im - b.im).abs() < 1e-10,
            "basis {basis}: {a:?} != {b:?}"
        );
    }
}

#[test]
fn remote_gate_becomes_local_and_restores_wire_order() {
    let device = Device::linear(4);
    let mut circuit = Circuit::new(4, 4);
    circuit.h(0).cx(0, 3);
    circuit.x(0).measure(3, 1);
    assert!(device.validate(&circuit).is_err());
    let result = device.prepare(&circuit).unwrap();
    assert_eq!(result.inserted_swaps, 4);
    assert_eq!(
        result.circuit.gates,
        vec![
            Gate::H(0),
            Gate::Swap(0, 1),
            Gate::Swap(1, 2),
            Gate::Cx(2, 3),
            Gate::Swap(1, 2),
            Gate::Swap(0, 1),
            Gate::X(0),
            Gate::Measure(3, 1),
        ]
    );
    device.validate(&result.circuit).unwrap();
    assert_eq!(circuit.len(), 4);
}

#[test]
fn routing_preserves_complex_amplitudes_for_all_basis_inputs() {
    // Test every ordered pair and all basis inputs, including reversed CX direction,
    // logical SWAPs, entanglement, relative phase, and subsequent operations on moved wires.
    for topo in [Topology::linear(4), Topology::grid(2, 2)] {
        for a in 0..4 {
            for b in 0..4 {
                if a == b {
                    continue;
                }
                for gate in [Gate::Cx(a, b), Gate::Cz(a, b), Gate::Swap(a, b)] {
                    let mut circuit = Circuit::new(4, 4);
                    circuit.h(a);
                    circuit.add(Gate::Ry(b, 0.73));
                    circuit.add(Gate::Rz(a, -0.42));
                    circuit.add(gate);
                    circuit.add(Gate::T(b));
                    circuit.cx(b, a);
                    let result = route_circuit(&circuit, &topo).unwrap();
                    for basis in 0..16 {
                        assert_same_state(&circuit, &result.circuit, basis);
                    }
                }
            }
        }
    }
}

#[test]
fn all_to_all_is_identity_and_resets_execution_pointer() {
    let mut circuit = qos_core::bell_circuit();
    circuit.pc = 2;
    let result = route_circuit(&circuit, &Topology::all_to_all(8)).unwrap();
    assert_eq!(result.inserted_swaps, 0);
    assert_eq!(result.circuit.gates, circuit.gates);
    assert_eq!(result.circuit.n_qubits, 2);
    assert_eq!(result.circuit.n_cbits, 2);
    assert_eq!(result.circuit.pc, 0);
    assert_eq!(circuit.pc, 2);
}

#[test]
fn disconnected_and_spare_wire_only_paths_are_rejected() {
    let mut circuit = Circuit::new(2, 0);
    circuit.cx(0, 1);
    for topology in [
        Topology {
            n_qubits: 2,
            connections: vec![vec![], vec![]],
        },
        Topology {
            n_qubits: 3,
            connections: vec![vec![2], vec![2], vec![0, 1]],
        },
    ] {
        assert!(matches!(
            route_circuit(&circuit, &topology),
            Err(BackendError::UnsupportedCircuit(_))
        ));
    }
}

#[test]
fn malformed_topologies_never_panic_or_look_fully_connected() {
    for topology in [
        Topology {
            n_qubits: 2,
            connections: vec![],
        },
        Topology {
            n_qubits: 2,
            connections: vec![vec![9], vec![0]],
        },
        Topology {
            n_qubits: 2,
            connections: vec![vec![1, 1], vec![0]],
        },
        Topology {
            n_qubits: 2,
            connections: vec![vec![1], vec![]],
        },
        Topology {
            n_qubits: 2,
            connections: vec![vec![0], vec![1]],
        },
    ] {
        assert!(matches!(topology.validate(), Err(BackendError::Config(_))));
        assert!(!topology.is_all_to_all());
        assert_eq!(topology.shortest_path(0, 1), None);
        let _ = topology.is_connected(0, 1);
        assert!(route_circuit(&qos_core::bell_circuit(), &topology).is_err());
    }
}

#[test]
fn invalid_ir_is_rejected_before_simulation() {
    let mut backend = LocalSimulatorBackend::new(3);
    let invalid = vec![
        Gate::H(3),
        Gate::Cx(0, 3),
        Gate::Cx(1, 1),
        Gate::Cz(2, 2),
        Gate::Swap(0, 0),
        Gate::Ccx(0, 1, 1),
        Gate::Ccx(0, 1, 3),
        Gate::Measure(3, 0),
        Gate::Measure(0, 2),
        Gate::Barrier(vec![3]),
        Gate::Reset(3),
        Gate::Rx(0, f64::NAN),
        Gate::Ry(0, f64::INFINITY),
        Gate::Rz(0, f64::NEG_INFINITY),
        Gate::U3(0, 0.0, f64::NAN, 0.0),
    ];
    for gate in invalid {
        let mut circuit = Circuit::new(3, 2);
        circuit.add(gate);
        assert!(matches!(
            backend.submit(&circuit, 1),
            Err(BackendError::UnsupportedCircuit(_))
        ));
    }
    // Rejections do not consume job IDs or create results.
    let id = backend.submit(&qos_core::bell_circuit(), 10).unwrap();
    assert_eq!(id, 1);
    assert_eq!(backend.result(id).unwrap().counts.values().sum::<u64>(), 10);
}

#[test]
fn native_gate_and_mid_circuit_measurement_constraints_are_enforced() {
    let mut device = Device::linear(3);
    device.0.native_gates.retain(|g| *g != GateType::Swap);
    let mut remote = Circuit::new(3, 0);
    remote.cx(0, 2);
    assert!(device.prepare(&remote).is_err()); // inserted SWAPs need native support
    device.0.native_gates = GateType::universal_set();
    device.0.mid_circuit_measurement = false;
    let mut circuit = qos_core::bell_circuit();
    circuit.add(Gate::Barrier(vec![0, 1]));
    device.prepare(&circuit).unwrap(); // terminal measurements and barriers are permitted
    circuit.x(0);
    assert!(device.prepare(&circuit).is_err());
    device.0.mid_circuit_measurement = true;
    device.prepare(&circuit).unwrap();
    device.0.native_gates.retain(|g| *g != GateType::X);
    assert!(device.prepare(&circuit).is_err());
}

#[test]
fn ccx_requires_pairwise_connectivity() {
    let mut circuit = Circuit::new(3, 0);
    circuit.add(Gate::Ccx(0, 1, 2));
    assert!(route_circuit(&circuit, &Topology::linear(3)).is_err());
    let routed = route_circuit(&circuit, &Topology::all_to_all(3)).unwrap();
    for basis in 0..8 {
        assert_same_state(&circuit, &routed.circuit, basis);
    }
}

#[test]
fn empty_and_oversized_circuits_and_no_topology_backend() {
    let mut device = Device::linear(0);
    let empty = device.prepare(&Circuit::new(0, 0)).unwrap();
    assert!(empty.circuit.is_empty());
    assert_eq!(empty.inserted_swaps, 0);
    assert!(device.prepare(&Circuit::new(1, 0)).is_err());
    device.0.max_qubits = 3;
    assert!(device.prepare(&Circuit::new(3, 0)).is_err()); // topology is still empty
    device.0.topology = None;
    let mut circuit = Circuit::new(3, 0);
    circuit.cx(0, 2);
    circuit.pc = 1;
    assert_eq!(device.prepare(&circuit).unwrap().circuit.pc, 0);
    circuit.add(Gate::H(3));
    assert!(device.prepare(&circuit).is_err());
}
