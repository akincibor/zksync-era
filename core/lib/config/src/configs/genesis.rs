use crate::configs::chain::L1BatchCommitDataGeneratorMode;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{Address, L1ChainId, L2ChainId, H256};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SharedBridge {
    pub bridgehub_proxy_addr: Address,
    pub state_transition_proxy_addr: Address,
    pub transparent_proxy_admin_addr: Address,
}

/// This config represents the genesis state of the chain.
/// Each chain has this config immutable and we update it only during the protocol upgrade
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenesisConfig {
    pub protocol_version: u16,
    pub genesis_root_hash: H256,
    pub rollup_last_leaf_index: u64,
    pub genesis_commitment: H256,
    pub bootloader_hash: H256,
    pub default_aa_hash: H256,
    pub l1_chain_id: L1ChainId,
    pub l2_chain_id: L2ChainId,
    pub recursion_node_level_vk_hash: H256,
    pub recursion_leaf_level_vk_hash: H256,
    pub recursion_circuits_set_vks_hash: H256,
    pub recursion_scheduler_level_vk_hash: H256,
    pub fee_account: Address,
    pub shared_bridge: Option<SharedBridge>,
    pub dummy_prover: bool,
    pub l1_batch_commit_data_generator_mode: L1BatchCommitDataGeneratorMode,
}

impl GenesisConfig {
    pub fn for_tests() -> Self {
        GenesisConfig {
            genesis_root_hash: H256::repeat_byte(0x01),
            rollup_last_leaf_index: 26,
            recursion_scheduler_level_vk_hash: H256::repeat_byte(0x02),
            fee_account: Default::default(),
            shared_bridge: Some(SharedBridge {
                bridgehub_proxy_addr: Address::repeat_byte(0x14),
                state_transition_proxy_addr: Address::repeat_byte(0x16),
                transparent_proxy_admin_addr: Address::repeat_byte(0x16),
            }),
            recursion_node_level_vk_hash: H256::repeat_byte(0x03),
            recursion_leaf_level_vk_hash: H256::repeat_byte(0x04),
            recursion_circuits_set_vks_hash: H256::repeat_byte(0x05),
            genesis_commitment: H256::repeat_byte(0x17),
            bootloader_hash: Default::default(),
            default_aa_hash: Default::default(),
            l1_chain_id: L1ChainId(9),
            protocol_version: 22,
            l2_chain_id: L2ChainId::default(),
            dummy_prover: false,
            l1_batch_commit_data_generator_mode: L1BatchCommitDataGeneratorMode::Rollup,
        }
    }
}
