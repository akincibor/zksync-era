use zk_evm_1_4_1::aux_structures::MemoryPage;
pub use zk_evm_1_4_1::zkevm_opcode_defs::system_params::{
    ERGS_PER_CIRCUIT, INITIAL_STORAGE_WRITE_PUBDATA_BYTES, MAX_PUBDATA_PER_BLOCK,
};
use zksync_system_constants::{
    MAX_L2_TX_GAS_LIMIT, MAX_NEW_FACTORY_DEPS, USED_1_4_1_BOOTLOADER_MEMORY_WORDS,
};

use crate::vm_latest::old_vm::utils::heap_page_from_base;

// The maximal number of transactions in a single batch
pub const MAX_TXS_IN_BLOCK: usize = 10000;

/// Max cycles for a single transaction.
pub const MAX_CYCLES_FOR_TX: u32 = u32::MAX;

/// The first 32 slots are reserved for debugging purposes
pub(crate) const DEBUG_SLOTS_OFFSET: usize = 8;
pub(crate) const DEBUG_FIRST_SLOTS: usize = 32;

/// The next 33 slots are reserved for dealing with the paymaster context (1 slot for storing length + 32 slots for storing the actual context).
pub(crate) const PAYMASTER_CONTEXT_SLOTS: usize = 32 + 1;
/// The next PAYMASTER_CONTEXT_SLOTS + 7 slots free slots are needed before each tx, so that the
/// postOp operation could be encoded correctly.
pub(crate) const MAX_POSTOP_SLOTS: usize = PAYMASTER_CONTEXT_SLOTS + 7;

/// Slots used to store the current L2 transaction's hash and the hash recommended
/// to be used for signing the transaction's content.
const CURRENT_L2_TX_HASHES_SLOTS: usize = 2;

/// Slots used to store the calldata for the KnownCodesStorage to mark new factory
/// dependencies as known ones. Besides the slots for the new factory dependencies themselves
/// another 4 slots are needed for: selector, marker of whether the user should pay for the pubdata,
/// the offset for the encoding of the array as well as the length of the array.
const NEW_FACTORY_DEPS_RESERVED_SLOTS: usize = MAX_NEW_FACTORY_DEPS + 4;

/// The operator can provide for each transaction the proposed minimal refund
pub(crate) const OPERATOR_REFUNDS_SLOTS: usize = MAX_TXS_IN_BLOCK;

pub(crate) const OPERATOR_REFUNDS_OFFSET: usize = DEBUG_SLOTS_OFFSET
    + DEBUG_FIRST_SLOTS
    + PAYMASTER_CONTEXT_SLOTS
    + CURRENT_L2_TX_HASHES_SLOTS
    + NEW_FACTORY_DEPS_RESERVED_SLOTS;

pub(crate) const TX_OVERHEAD_OFFSET: usize = OPERATOR_REFUNDS_OFFSET + OPERATOR_REFUNDS_SLOTS;
pub(crate) const TX_OVERHEAD_SLOTS: usize = MAX_TXS_IN_BLOCK;

pub(crate) const TX_TRUSTED_GAS_LIMIT_OFFSET: usize = TX_OVERHEAD_OFFSET + TX_OVERHEAD_SLOTS;
pub(crate) const TX_TRUSTED_GAS_LIMIT_SLOTS: usize = MAX_TXS_IN_BLOCK;

pub(crate) const COMPRESSED_BYTECODES_SLOTS: usize = 32768;

pub(crate) const PRIORITY_TXS_L1_DATA_OFFSET: usize =
    COMPRESSED_BYTECODES_OFFSET + COMPRESSED_BYTECODES_SLOTS;
pub(crate) const PRIORITY_TXS_L1_DATA_SLOTS: usize = 2;

pub const OPERATOR_PROVIDED_L1_MESSENGER_PUBDATA_OFFSET: usize =
    PRIORITY_TXS_L1_DATA_OFFSET + PRIORITY_TXS_L1_DATA_SLOTS;

/// One of "worst case" scenarios for the number of state diffs in a batch is when 120kb of pubdata is spent
/// on repeated writes, that are all zeroed out. In this case, the number of diffs is 120k / 5 = 24k. This means that they will have
/// accommodate 6528000 bytes of calldata for the uncompressed state diffs. Adding 120k on top leaves us with
/// roughly 6650000 bytes needed for calldata. 207813 slots are needed to accommodate this amount of data.
/// We round up to 208000 slots just in case.
///
/// In theory though much more calldata could be used (if for instance 1 byte is used for enum index). It is the responsibility of the
/// operator to ensure that it can form the correct calldata for the L1Messenger.
pub(crate) const OPERATOR_PROVIDED_L1_MESSENGER_PUBDATA_SLOTS: usize = 208000;

pub(crate) const BOOTLOADER_TX_DESCRIPTION_OFFSET: usize =
    OPERATOR_PROVIDED_L1_MESSENGER_PUBDATA_OFFSET + OPERATOR_PROVIDED_L1_MESSENGER_PUBDATA_SLOTS;

/// The size of the bootloader memory dedicated to the encodings of transactions
pub(crate) const BOOTLOADER_TX_ENCODING_SPACE: u32 =
    (USED_1_4_1_BOOTLOADER_MEMORY_WORDS - TX_DESCRIPTION_OFFSET - MAX_TXS_IN_BLOCK) as u32;

// Size of the bootloader tx description in words
pub(crate) const BOOTLOADER_TX_DESCRIPTION_SIZE: usize = 2;

/// The actual descriptions of transactions should start after the minor descriptions and a MAX_POSTOP_SLOTS
/// free slots to allow postOp encoding.
pub(crate) const TX_DESCRIPTION_OFFSET: usize = BOOTLOADER_TX_DESCRIPTION_OFFSET
    + BOOTLOADER_TX_DESCRIPTION_SIZE * MAX_TXS_IN_BLOCK
    + MAX_POSTOP_SLOTS;

pub(crate) const TX_GAS_LIMIT_OFFSET: usize = 4;

const INITIAL_BASE_PAGE: u32 = 8;
pub const BOOTLOADER_HEAP_PAGE: u32 = heap_page_from_base(MemoryPage(INITIAL_BASE_PAGE)).0;

/// VM Hooks are used for communication between bootloader and tracers.
/// The 'type' / 'opcode' is put into VM_HOOK_POSITION slot,
/// and VM_HOOKS_PARAMS_COUNT parameters (each 32 bytes) are put in the slots before.
/// So the layout looks like this:
/// `[param 0][param 1][vmhook opcode]`
pub const VM_HOOK_POSITION: u32 = RESULT_SUCCESS_FIRST_SLOT - 1;
pub const VM_HOOK_PARAMS_COUNT: u32 = 2;
pub const VM_HOOK_PARAMS_START_POSITION: u32 = VM_HOOK_POSITION - VM_HOOK_PARAMS_COUNT;

pub(crate) const MAX_MEM_SIZE_BYTES: u32 = 24000000;

/// Arbitrary space in memory closer to the end of the page
pub const RESULT_SUCCESS_FIRST_SLOT: u32 =
    (MAX_MEM_SIZE_BYTES - (MAX_TXS_IN_BLOCK as u32) * 32) / 32;

/// How many gas bootloader is allowed to spend within one block.
/// Note that this value doesn't correspond to the gas limit of any particular transaction
/// (except for the fact that, of course, gas limit for each transaction should be <= `BLOCK_GAS_LIMIT`).
pub const BLOCK_GAS_LIMIT: u32 =
    zk_evm_1_4_1::zkevm_opcode_defs::system_params::VM_INITIAL_FRAME_ERGS;

/// How many gas is allowed to spend on a single transaction in eth_call method
pub const ETH_CALL_GAS_LIMIT: u32 = MAX_L2_TX_GAS_LIMIT as u32;

/// ID of the transaction from L1
pub const L1_TX_TYPE: u8 = 255;

pub(crate) const TX_OPERATOR_L2_BLOCK_INFO_OFFSET: usize =
    TX_TRUSTED_GAS_LIMIT_OFFSET + TX_TRUSTED_GAS_LIMIT_SLOTS;

pub(crate) const TX_OPERATOR_SLOTS_PER_L2_BLOCK_INFO: usize = 4;
pub(crate) const TX_OPERATOR_L2_BLOCK_INFO_SLOTS: usize =
    (MAX_TXS_IN_BLOCK + 1) * TX_OPERATOR_SLOTS_PER_L2_BLOCK_INFO;

pub(crate) const COMPRESSED_BYTECODES_OFFSET: usize =
    TX_OPERATOR_L2_BLOCK_INFO_OFFSET + TX_OPERATOR_L2_BLOCK_INFO_SLOTS;

/// The maximal gas limit that gets passed into an L1->L2 transaction
pub(crate) const PRIORITY_TX_MAX_GAS_LIMIT: usize = 72_000_000;

/// The amount of gas to be charged for occupying a single slot of a transaction.
pub(crate) const TX_SLOT_OVERHEAD_GAS: u32 = 10_000;

/// The amount of gas to be charged for occupying a single byte of the bootloader's memory.
pub(crate) const TX_MEMORY_OVERHEAD_GAS: u32 = 10;
