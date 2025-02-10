use borsh::{BorshDeserialize, BorshSerialize};
use enum_map::{enum_map, Enum, EnumMap};
use near_parameters::{ActionCosts, ExtCosts, ExtCostsConfig};
use near_primitives_core::types::{Compute, Gas};
use std::fmt;
use strum::IntoEnumIterator;

/// Profile of gas consumption.
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProfileDataV3 {
    /// Gas spent on sending or executing actions.
    #[serde(skip)] // FIXME: add serde
    pub actions_profile: EnumMap<ActionCosts, Gas>,
    /// Non-action gas spent outside the WASM VM while executing a contract.
    #[serde(skip)] // FIXME: add serde
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

    /// Test instance with unique numbers in each field.
    pub fn test() -> Self {
        let mut profile_data = ProfileDataV3::default();
        for (i, cost) in ExtCosts::iter().enumerate() {
            profile_data.add_ext_cost(cost, i as Gas);
        }
        for (i, cost) in ActionCosts::iter().enumerate() {
            profile_data.add_action_cost(cost, i as Gas + 1000);
        }
        profile_data
    }

    #[inline]
    pub fn merge(&mut self, other: &ProfileDataV3) {
        for ((_, gas), (_, other_gas)) in
            self.actions_profile.iter_mut().zip(other.actions_profile.iter())
        {
            *gas = gas.saturating_add(*other_gas);
        }
        for ((_, gas), (_, other_gas)) in
            self.wasm_ext_profile.iter_mut().zip(other.wasm_ext_profile.iter())
        {
            *gas = gas.saturating_add(*other_gas);
        }
        self.wasm_gas = self.wasm_gas.saturating_add(other.wasm_gas);
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
        self.wasm_gas =
            total_gas_burnt.saturating_sub(self.action_gas()).saturating_sub(self.host_gas());
    }

    pub fn get_action_cost(&self, action: ActionCosts) -> Gas {
        self.actions_profile[action]
    }

    pub fn get_ext_cost(&self, ext: ExtCosts) -> Gas {
        self.wasm_ext_profile[ext]
    }

    pub fn get_wasm_cost(&self) -> Gas {
        self.wasm_gas
    }

    fn host_gas(&self) -> Gas {
        self.wasm_ext_profile.as_slice().iter().copied().fold(0, Gas::saturating_add)
    }

    pub fn action_gas(&self) -> Gas {
        self.actions_profile.as_slice().iter().copied().fold(0, Gas::saturating_add)
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
        ext_compute_cost.saturating_add(self.action_gas()).saturating_add(self.get_wasm_cost())
    }
}

impl BorshDeserialize for ProfileDataV3 {
    fn deserialize_reader<R: std::io::Read>(rd: &mut R) -> std::io::Result<Self> {
        let actions_array: Vec<u64> = BorshDeserialize::deserialize_reader(rd)?;
        let ext_array: Vec<u64> = BorshDeserialize::deserialize_reader(rd)?;
        let wasm_gas: u64 = BorshDeserialize::deserialize_reader(rd)?;

        // Mapping raw arrays to enum maps.
        // The enum map could be smaller or larger than the raw array.
        // Extra values in the array that are unknown to the current binary will
        // be ignored. Missing values are filled with 0.
        let actions_profile = enum_map! {
            cost => actions_array.get(borsh_action_index(cost)).copied().unwrap_or(0)
        };
        let wasm_ext_profile = enum_map! {
            cost => ext_array.get(borsh_ext_index(cost)).copied().unwrap_or(0)
        };

        Ok(Self { actions_profile, wasm_ext_profile, wasm_gas })
    }
}

impl BorshSerialize for ProfileDataV3 {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        let mut actions_costs: Vec<u64> = vec![0u64; ActionCosts::LENGTH];
        for (cost, gas) in self.actions_profile.iter() {
            actions_costs[borsh_action_index(cost)] = *gas;
        }
        BorshSerialize::serialize(&actions_costs, writer)?;

        let mut ext_costs: Vec<u64> = vec![0u64; ExtCosts::LENGTH];
        for (cost, gas) in self.wasm_ext_profile.iter() {
            ext_costs[borsh_ext_index(cost)] = *gas;
        }
        BorshSerialize::serialize(&ext_costs, writer)?;

        let wasm_cost: u64 = self.wasm_gas;
        BorshSerialize::serialize(&wasm_cost, writer)
    }
}

/// Fixed index of an action cost for borsh (de)serialization.
///
/// We use borsh to store profiles on the DB and borsh is quite fragile with
/// changes. This mapping is to decouple the Rust enum from the borsh
/// representation. The enum can be changed freely but here in the mapping we
/// can only append more values at the end.
///
/// TODO: Consider changing this to a different format (e.g. protobuf) because
/// canonical representation is not required here.
const fn borsh_action_index(action: ActionCosts) -> usize {
    // actual indices are defined on the enum variants
    action as usize
}

/// Fixed index of an ext cost for borsh (de)serialization.
///
/// We use borsh to store profiles on the DB and borsh is quite fragile with
/// changes. This mapping is to decouple the Rust enum from the borsh
/// representation. The enum can be changed freely but here in the mapping we
/// can only append more values at the end.
///
/// TODO: Consider changing this to a different format (e.g. protobuf) because
/// canonical representation is not required here.
const fn borsh_ext_index(ext: ExtCosts) -> usize {
    // actual indices are defined on the enum variants
    ext as usize
}

impl fmt::Debug for ProfileDataV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use num_rational::Ratio;
        let host_gas = self.host_gas();
        let action_gas = self.action_gas();

        writeln!(f, "------------------------------")?;
        writeln!(f, "Action gas: {}", action_gas)?;
        writeln!(f, "------ Host functions --------")?;
        for cost in ExtCosts::iter() {
            let d = self.get_ext_cost(cost);
            if d != 0 {
                writeln!(
                    f,
                    "{} -> {} [{}% host]",
                    cost,
                    d,
                    Ratio::new(d * 100, core::cmp::max(host_gas, 1)).to_integer(),
                )?;
            }
        }
        writeln!(f, "------ Actions --------")?;
        for cost in ActionCosts::iter() {
            let d = self.get_action_cost(cost);
            if d != 0 {
                writeln!(f, "{} -> {}", cost, d)?;
            }
        }
        writeln!(f, "------------------------------")?;
        Ok(())
    }
}
