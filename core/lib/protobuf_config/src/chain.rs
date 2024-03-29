use anyhow::Context as _;
use zksync_config::configs;
use zksync_protobuf::{repr::ProtoRepr, required};

use crate::proto::chain as proto;

impl proto::FeeModelVersion {
    fn new(n: &configs::chain::FeeModelVersion) -> Self {
        use configs::chain::FeeModelVersion as From;
        match n {
            From::V1 => Self::V1,
            From::V2 => Self::V2,
        }
    }

    fn parse(&self) -> configs::chain::FeeModelVersion {
        use configs::chain::FeeModelVersion as To;
        match self {
            Self::V1 => To::V1,
            Self::V2 => To::V2,
        }
    }
}

impl proto::L1BatchCommitDataGeneratorMode {
    fn new(n: &configs::chain::L1BatchCommitDataGeneratorMode) -> Self {
        use configs::chain::L1BatchCommitDataGeneratorMode as From;
        match n {
            From::Rollup => Self::Rollup,
            From::Validium => Self::Validium,
        }
    }

    fn parse(&self) -> configs::chain::L1BatchCommitDataGeneratorMode {
        use configs::chain::L1BatchCommitDataGeneratorMode as To;
        match self {
            Self::Rollup => To::Rollup,
            Self::Validium => To::Validium,
        }
    }
}
impl ProtoRepr for proto::StateKeeper {
    type Type = configs::chain::StateKeeperConfig;
    fn read(&self) -> anyhow::Result<Self::Type> {
        #[allow(deprecated)]
        Ok(Self::Type {
            transaction_slots: required(&self.transaction_slots)
                .and_then(|x| Ok((*x).try_into()?))
                .context("transaction_slots")?,
            block_commit_deadline_ms: *required(&self.block_commit_deadline_ms)
                .context("block_commit_deadline_ms")?,
            miniblock_commit_deadline_ms: *required(&self.miniblock_commit_deadline_ms)
                .context("miniblock_commit_deadline_ms")?,
            miniblock_seal_queue_capacity: required(&self.miniblock_seal_queue_capacity)
                .and_then(|x| Ok((*x).try_into()?))
                .context("miniblock_seal_queue_capacity")?,
            max_single_tx_gas: *required(&self.max_single_tx_gas).context("max_single_tx_gas")?,
            max_allowed_l2_tx_gas_limit: *required(&self.max_allowed_l2_tx_gas_limit)
                .context("max_allowed_l2_tx_gas_limit")?,
            reject_tx_at_geometry_percentage: *required(&self.reject_tx_at_geometry_percentage)
                .context("reject_tx_at_geometry_percentage")?,
            reject_tx_at_eth_params_percentage: *required(&self.reject_tx_at_eth_params_percentage)
                .context("reject_tx_at_eth_params_percentage")?,
            reject_tx_at_gas_percentage: *required(&self.reject_tx_at_gas_percentage)
                .context("reject_tx_at_gas_percentage")?,
            close_block_at_geometry_percentage: *required(&self.close_block_at_geometry_percentage)
                .context("close_block_at_geometry_percentage")?,
            close_block_at_eth_params_percentage: *required(
                &self.close_block_at_eth_params_percentage,
            )
            .context("close_block_at_eth_params_percentage")?,
            close_block_at_gas_percentage: *required(&self.close_block_at_gas_percentage)
                .context("close_block_at_gas_percentage")?,
            minimal_l2_gas_price: *required(&self.minimal_l2_gas_price)
                .context("minimal_l2_gas_price")?,
            compute_overhead_part: *required(&self.compute_overhead_part)
                .context("compute_overhead_part")?,
            pubdata_overhead_part: *required(&self.pubdata_overhead_part)
                .context("pubdata_overhead_part")?,
            batch_overhead_l1_gas: *required(&self.batch_overhead_l1_gas)
                .context("batch_overhead_l1_gas")?,
            max_gas_per_batch: *required(&self.max_gas_per_batch).context("max_gas_per_batch")?,
            max_pubdata_per_batch: *required(&self.max_pubdata_per_batch)
                .context("max_pubdata_per_batch")?,
            fee_model_version: required(&self.fee_model_version)
                .and_then(|x| Ok(proto::FeeModelVersion::try_from(*x)?))
                .context("fee_model_version")?
                .parse(),
            validation_computational_gas_limit: *required(&self.validation_computational_gas_limit)
                .context("validation_computational_gas_limit")?,
            save_call_traces: *required(&self.save_call_traces).context("save_call_traces")?,
            virtual_blocks_interval: *required(&self.virtual_blocks_interval)
                .context("virtual_blocks_interval")?,
            virtual_blocks_per_miniblock: *required(&self.virtual_blocks_per_miniblock)
                .context("virtual_blocks_per_miniblock")?,
            enum_index_migration_chunk_size: self
                .enum_index_migration_chunk_size
                .map(|x| x.try_into())
                .transpose()
                .context("enum_index_migration_chunk_size")?,
            l1_batch_commit_data_generator_mode: required(
                &self.l1_batch_commit_data_generator_mode,
            )
            .and_then(|x| Ok(proto::L1BatchCommitDataGeneratorMode::try_from(*x)?))
            .context("l1_batch_commit_data_generator_mode")?
            .parse(),

            // We need these values only for instantiating configs from envs, so it's not
            // needed during the initialization from files
            bootloader_hash: None,
            default_aa_hash: None,
            fee_account_addr: None,
        })
    }

    fn build(this: &Self::Type) -> Self {
        Self {
            transaction_slots: Some(this.transaction_slots.try_into().unwrap()),
            block_commit_deadline_ms: Some(this.block_commit_deadline_ms),
            miniblock_commit_deadline_ms: Some(this.miniblock_commit_deadline_ms),
            miniblock_seal_queue_capacity: Some(
                this.miniblock_seal_queue_capacity.try_into().unwrap(),
            ),
            max_single_tx_gas: Some(this.max_single_tx_gas),
            max_allowed_l2_tx_gas_limit: Some(this.max_allowed_l2_tx_gas_limit),
            reject_tx_at_geometry_percentage: Some(this.reject_tx_at_geometry_percentage),
            reject_tx_at_eth_params_percentage: Some(this.reject_tx_at_eth_params_percentage),
            reject_tx_at_gas_percentage: Some(this.reject_tx_at_gas_percentage),
            close_block_at_geometry_percentage: Some(this.close_block_at_geometry_percentage),
            close_block_at_eth_params_percentage: Some(this.close_block_at_eth_params_percentage),
            close_block_at_gas_percentage: Some(this.close_block_at_gas_percentage),
            minimal_l2_gas_price: Some(this.minimal_l2_gas_price),
            compute_overhead_part: Some(this.compute_overhead_part),
            pubdata_overhead_part: Some(this.pubdata_overhead_part),
            batch_overhead_l1_gas: Some(this.batch_overhead_l1_gas),
            max_gas_per_batch: Some(this.max_gas_per_batch),
            max_pubdata_per_batch: Some(this.max_pubdata_per_batch),
            fee_model_version: Some(proto::FeeModelVersion::new(&this.fee_model_version).into()),
            validation_computational_gas_limit: Some(this.validation_computational_gas_limit),
            save_call_traces: Some(this.save_call_traces),
            virtual_blocks_interval: Some(this.virtual_blocks_interval),
            virtual_blocks_per_miniblock: Some(this.virtual_blocks_per_miniblock),
            enum_index_migration_chunk_size: this
                .enum_index_migration_chunk_size
                .as_ref()
                .map(|x| (*x).try_into().unwrap()),
            l1_batch_commit_data_generator_mode: Some(
                proto::L1BatchCommitDataGeneratorMode::new(
                    &this.l1_batch_commit_data_generator_mode,
                )
                .into(),
            ),
        }
    }
}

impl ProtoRepr for proto::OperationsManager {
    type Type = configs::chain::OperationsManagerConfig;
    fn read(&self) -> anyhow::Result<Self::Type> {
        Ok(Self::Type {
            delay_interval: *required(&self.delay_interval).context("delay_interval")?,
        })
    }

    fn build(this: &Self::Type) -> Self {
        Self {
            delay_interval: Some(this.delay_interval),
        }
    }
}

impl ProtoRepr for proto::Mempool {
    type Type = configs::chain::MempoolConfig;
    fn read(&self) -> anyhow::Result<Self::Type> {
        Ok(Self::Type {
            sync_interval_ms: *required(&self.sync_interval_ms).context("sync_interval_ms")?,
            sync_batch_size: required(&self.sync_batch_size)
                .and_then(|x| Ok((*x).try_into()?))
                .context("sync_batch_size")?,
            capacity: *required(&self.capacity).context("capacity")?,
            stuck_tx_timeout: *required(&self.stuck_tx_timeout).context("stuck_tx_timeout")?,
            remove_stuck_txs: *required(&self.remove_stuck_txs).context("remove_stuck_txs")?,
            delay_interval: *required(&self.delay_interval).context("delay_interval")?,
        })
    }

    fn build(this: &Self::Type) -> Self {
        Self {
            sync_interval_ms: Some(this.sync_interval_ms),
            sync_batch_size: Some(this.sync_batch_size.try_into().unwrap()),
            capacity: Some(this.capacity),
            stuck_tx_timeout: Some(this.stuck_tx_timeout),
            remove_stuck_txs: Some(this.remove_stuck_txs),
            delay_interval: Some(this.delay_interval),
        }
    }
}
