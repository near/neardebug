use enum_map::{enum_map, EnumArray, EnumMap};
use near_parameters::{ActionCosts, ExtCosts, ExtCostsConfig};
use near_primitives_core::types::{Compute, Gas};
use serde::ser::SerializeMap;
use serde::Serializer;
use std::fmt::Display;

fn serialize_enum_map<K: EnumArray<u64> + Display, S: Serializer>(
    map: &EnumMap<K, Gas>,
    ser: S,
) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> {
    let mut outmap = ser.serialize_map(Some(map.len()))?;
    for (k, v) in map {
        outmap.serialize_entry(&k.to_string(), v)?;
    }
    outmap.end()
}

/// Profile of gas consumption.
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProfileDataV3 {
    /// Gas spent on sending or executing actions.
    #[serde(serialize_with = "serialize_enum_map")]
    pub actions_profile: EnumMap<ActionCosts, Gas>,
    /// Non-action gas spent outside the WASM VM while executing a contract.
    #[serde(serialize_with = "serialize_enum_map")]
    pub wasm_ext_profile: EnumMap<ExtCosts, Gas>,
    /// Gas spent on execution inside the WASM VM.
    pub wasm_gas: Gas,
}

impl Default for ProfileDataV3 {
    fn default() -> ProfileDataV3 {
        ProfileDataV3::new()
    }
}

impl ProfileDataV3 {
    #[inline]
    pub fn new() -> Self {
        Self {
            actions_profile: enum_map! { _ => 0 },
            wasm_ext_profile: enum_map! { _ => 0 },
            wasm_gas: 0,
        }
    }

    #[inline]
    pub fn add_action_cost(&mut self, action: ActionCosts, value: Gas) {
        self.actions_profile[action] = self.actions_profile[action].saturating_add(value);
    }

    #[inline]
    pub fn add_ext_cost(&mut self, ext: ExtCosts, value: Gas) {
        self.wasm_ext_profile[ext] = self.wasm_ext_profile[ext].saturating_add(value);
    }

    /// WasmInstruction is the only cost we don't explicitly account for.
    /// Instead, we compute it at the end of contract call as the difference
    /// between total gas burnt and what we've explicitly accounted for in the
    /// profile.
    ///
    /// This is because WasmInstruction is the hottest cost and is implemented
    /// with the help on the VM side, so we don't want to have profiling logic
    /// there both for simplicity and efficiency reasons.
    pub fn compute_wasm_instruction_cost(&mut self, total_gas_burnt: Gas) {
        self.wasm_gas = total_gas_burnt
            .saturating_sub(self.action_gas())
            .saturating_sub(self.host_gas());
    }

    pub fn get_wasm_cost(&self) -> Gas {
        self.wasm_gas
    }

    fn host_gas(&self) -> Gas {
        self.wasm_ext_profile
            .as_slice()
            .iter()
            .copied()
            .fold(0, Gas::saturating_add)
    }

    pub fn action_gas(&self) -> Gas {
        self.actions_profile
            .as_slice()
            .iter()
            .copied()
            .fold(0, Gas::saturating_add)
    }

    /// Returns total compute usage of host calls.
    pub fn total_compute_usage(&self, ext_costs_config: &ExtCostsConfig) -> Compute {
        let ext_compute_cost = self
            .wasm_ext_profile
            .iter()
            .map(|(key, value)| {
                // Technically, gas cost might be zero while the compute cost is non-zero. To
                // handle this case, we would need to explicitly count number of calls, not just
                // the total gas usage.
                // We don't have such costs at the moment, so this case is not implemented.
                debug_assert!(key.gas(ext_costs_config) > 0 || key.compute(ext_costs_config) == 0);

                if *value == 0 {
                    return *value;
                }
                // If the `value` is non-zero, the gas cost also must be non-zero.
                debug_assert!(key.gas(ext_costs_config) != 0);
                ((*value as u128).saturating_mul(key.compute(ext_costs_config) as u128)
                    / (key.gas(ext_costs_config) as u128)) as u64
            })
            .fold(0, Compute::saturating_add);

        // We currently only support compute costs for host calls. In the future we might add
        // them for actions as well.
        ext_compute_cost
            .saturating_add(self.action_gas())
            .saturating_add(self.get_wasm_cost())
    }
}
