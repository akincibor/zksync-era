use crate::proto::general as proto;
use anyhow::Context as _;

use crate::read_optional_repr;
use zksync_config::configs::chain::{
    CircuitBreakerConfig, MempoolConfig, OperationsManagerConfig, StateKeeperConfig,
};
use zksync_config::configs::fri_prover_group::FriProverGroupConfig;
use zksync_config::configs::house_keeper::HouseKeeperConfig;
use zksync_config::configs::{
    FriProofCompressorConfig, FriProverConfig, FriProverGatewayConfig, FriWitnessGeneratorConfig,
    FriWitnessVectorGeneratorConfig, General, PrometheusConfig, ProofDataHandlerConfig,
};
use zksync_config::{ApiConfig, DBConfig, ETHConfig, PostgresConfig};
use zksync_protobuf::{ProtoFmt, ProtoRepr};

impl ProtoRepr for proto::GeneralConfig {
    type Type = General;

    fn read(&self) -> anyhow::Result<Self::Type> {
        Ok(Self::Type {
            postgres_config: read_optional_repr(&self.postgres).context("postgres")?,
            circuit_breaker_config: read_optional_repr(&self.circuit_breaker)
                .context("circuit_breaker")?,
            mempool_config: read_optional_repr(&self.mempool).context("mempool")?,
            operations_manager_config: read_optional_repr(&self.operations_manager)
                .context("operations_manager")?,
            state_keeper_config: read_optional_repr(&self.state_keeper).context("state_keeper")?,
            house_keeper_config: read_optional_repr(&self.house_keeper).context("house_keeper")?,
            proof_compressor_config: read_optional_repr(&self.proof_compressor)
                .context("fri_proof_compressor")?,
            prover_config: read_optional_repr(&self.prover).context("fri_prover")?,
            prover_gateway: read_optional_repr(&self.prover_gateway).context("fri_prover")?,
            witness_vector_generator: read_optional_repr(&self.witness_vector_generator)
                .context("fri_prover")?,
            prover_group_config: read_optional_repr(&self.prover_group)
                .context("fri_prover_group")?,
            prometheus_config: read_optional_repr(&self.prometheus).context("prometheus")?,
            proof_data_handler_config: read_optional_repr(&self.data_handler)
                .context("proof_data_handler")?,
            witness_generator: read_optional_repr(&self.witness_generator)
                .context("witness_generator")?,
            api_config: read_optional_repr(&self.api).context("api")?,
            db_config: read_optional_repr(&self.db).context("db")?,
            eth: read_optional_repr(&self.eth).context("db")?,
        })
    }

    fn build(this: &Self::Type) -> Self {
        Self {
            postgres: this.postgres_config.as_ref().map(ProtoRepr::build),
            circuit_breaker: this.circuit_breaker_config.as_ref().map(ProtoRepr::build),
            mempool: this.mempool_config.as_ref().map(ProtoRepr::build),
            operations_manager: this
                .operations_manager_config
                .as_ref()
                .map(ProtoRepr::build),
            state_keeper: this.state_keeper_config.as_ref().map(ProtoRepr::build),
            house_keeper: this.house_keeper_config.as_ref().map(ProtoRepr::build),
            proof_compressor: this.proof_compressor_config.as_ref().map(ProtoRepr::build),
            prover: this.prover_config.as_ref().map(ProtoRepr::build),
            prover_group: this.prover_group_config.as_ref().map(ProtoRepr::build),
            witness_generator: this.witness_generator.as_ref().map(ProtoRepr::build),
            prover_gateway: this.prover_gateway.as_ref().map(ProtoRepr::build),
            witness_vector_generator: this.witness_vector_generator.as_ref().map(ProtoRepr::build),
            prometheus: this.prometheus_config.as_ref().map(ProtoRepr::build),
            data_handler: this
                .proof_data_handler_config
                .as_ref()
                .map(ProtoRepr::build),
            api: this.api_config.as_ref().map(ProtoRepr::build),
            db: this.db_config.as_ref().map(ProtoRepr::build),
            eth: this.eth.as_ref().map(ProtoRepr::build),
        }
    }
}
