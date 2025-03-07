use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::TryInto,
    fmt, mem,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use multivm::{
    interface::{
        ExecutionResult, L1BatchEnv, SystemEnv, TxExecutionMode, VmExecutionResultAndLogs,
    },
    vm_latest::constants::BLOCK_GAS_LIMIT,
};
use tokio::sync::{mpsc, watch};
use zksync_contracts::BaseSystemContracts;
use zksync_types::{
    block::MiniblockExecutionData, fee_model::BatchFeeInput, protocol_upgrade::ProtocolUpgradeTx,
    Address, L1BatchNumber, L2ChainId, MiniblockNumber, ProtocolVersionId, Transaction, H256,
};

use crate::{
    state_keeper::{
        batch_executor::{BatchExecutor, BatchExecutorHandle, Command, TxExecutionResult},
        io::{IoCursor, L1BatchParams, MiniblockParams, PendingBatchData, StateKeeperIO},
        seal_criteria::{IoSealCriteria, SequencerSealer},
        tests::{default_l1_batch_env, default_vm_block_result, BASE_SYSTEM_CONTRACTS},
        types::ExecutionMetricsForCriteria,
        updates::UpdatesManager,
        OutputHandler, StateKeeperOutputHandler, ZkSyncStateKeeper,
    },
    utils::testonly::create_l2_transaction,
};

const FEE_ACCOUNT: Address = Address::repeat_byte(0x11);

/// Main entry for writing tests for the state keeper.
/// Represents a planned sequence of actions that would happen with the state keeper.
/// We defined a scenario by telling *exactly* what we expect to happen, and then launch the state keeper.
/// While state keeper progresses over the planned transactions, `TestScenario` makes sure that every action happens
/// according to the scenario.
///
/// Every action requires a description: since in most scenarios there will be a lot of similar actions (e.g. `next_tx`
/// or `seal_miniblock`) it helps to see which action *exactly* caused a test failure. It's recommended to write
/// descriptions that are not only unique, but also will explain *why* we expected this action to happen. This way,
/// it would be easier for developer to find the problem.
///
/// See any test in the `mod.rs` file to get a visual example.
pub(crate) struct TestScenario {
    actions: VecDeque<ScenarioItem>,
    pending_batch: Option<PendingBatchData>,
    l1_batch_seal_fn: Box<SealFn>,
    miniblock_seal_fn: Box<SealFn>,
}

type SealFn = dyn FnMut(&UpdatesManager) -> bool + Send;

impl fmt::Debug for TestScenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestScenario")
            .field("actions", &self.actions)
            .field("pending_batch", &self.pending_batch)
            .finish_non_exhaustive()
    }
}

impl TestScenario {
    pub(crate) fn new() -> Self {
        Self {
            actions: VecDeque::new(),
            pending_batch: None,
            l1_batch_seal_fn: Box::new(|_| false),
            miniblock_seal_fn: Box::new(|_| false),
        }
    }

    /// Adds a pending batch data that would be fed into the state keeper.
    /// Note that during processing pending batch, state keeper do *not* call `seal_miniblock` method on the IO (since
    /// it only recovers the temporary state).
    pub(crate) fn load_pending_batch(mut self, pending_batch: PendingBatchData) -> Self {
        self.pending_batch = Some(pending_batch);
        self
    }

    /// Configures scenario to repeatedly return `None` to tx requests until the next action from the scenario happens.
    pub(crate) fn no_txs_until_next_action(mut self, description: &'static str) -> Self {
        self.actions
            .push_back(ScenarioItem::NoTxsUntilNextAction(description));
        self
    }

    /// Increments protocol version returned by IO.
    pub(crate) fn increment_protocol_version(mut self, description: &'static str) -> Self {
        self.actions
            .push_back(ScenarioItem::IncrementProtocolVersion(description));
        self
    }

    /// Expect the state keeper to request a transaction from IO.
    /// Adds both a transaction and an outcome of this transaction (that would be returned to the state keeper from the
    /// batch executor).
    pub(crate) fn next_tx(
        mut self,
        description: &'static str,
        tx: Transaction,
        result: TxExecutionResult,
    ) -> Self {
        self.actions
            .push_back(ScenarioItem::Tx(description, tx, result));
        self
    }

    /// Expect the state keeper to rollback the transaction (i.e. return to the mempool).
    pub(crate) fn tx_rollback(mut self, description: &'static str, tx: Transaction) -> Self {
        self.actions
            .push_back(ScenarioItem::Rollback(description, tx));
        self
    }

    /// Expect the state keeper to reject the transaction.
    /// `err` argument is an optional substring of the expected error message. If `None` is provided, any rejection
    /// would work. If `Some` is provided, rejection reason would be checked against the provided substring.
    pub(crate) fn tx_rejected(
        mut self,
        description: &'static str,
        tx: Transaction,
        err: Option<String>,
    ) -> Self {
        self.actions
            .push_back(ScenarioItem::Reject(description, tx, err));
        self
    }

    /// Expects the miniblock to be sealed.
    pub(crate) fn miniblock_sealed(mut self, description: &'static str) -> Self {
        self.actions
            .push_back(ScenarioItem::MiniblockSeal(description, None));
        self
    }

    /// Expects the miniblock to be sealed.
    /// Accepts a function that would be given access to the received miniblock seal params, which can implement
    /// additional assertions on the sealed miniblock.
    pub(crate) fn miniblock_sealed_with<F: FnOnce(&UpdatesManager) + Send + 'static>(
        mut self,
        description: &'static str,
        f: F,
    ) -> Self {
        self.actions
            .push_back(ScenarioItem::MiniblockSeal(description, Some(Box::new(f))));
        self
    }

    /// Expects the batch to be sealed.
    pub(crate) fn batch_sealed(mut self, description: &'static str) -> Self {
        self.actions
            .push_back(ScenarioItem::BatchSeal(description, None));
        self
    }

    /// Expects the batch to be sealed.
    /// Accepts a function that would be given access to the received batch seal params, which can implement
    /// additional assertions on the sealed batch.
    pub(crate) fn batch_sealed_with<F>(mut self, description: &'static str, f: F) -> Self
    where
        F: FnOnce(&UpdatesManager) + Send + 'static,
    {
        self.actions
            .push_back(ScenarioItem::BatchSeal(description, Some(Box::new(f))));
        self
    }

    pub(crate) fn seal_l1_batch_when<F>(mut self, seal_fn: F) -> Self
    where
        F: FnMut(&UpdatesManager) -> bool + Send + 'static,
    {
        self.l1_batch_seal_fn = Box::new(seal_fn);
        self
    }

    pub(crate) fn seal_miniblock_when<F>(mut self, seal_fn: F) -> Self
    where
        F: FnMut(&UpdatesManager) -> bool + Send + 'static,
    {
        self.miniblock_seal_fn = Box::new(seal_fn);
        self
    }

    /// Launches the test.
    /// Provided `SealManager` is expected to be externally configured to adhere the written scenario logic.
    pub(crate) async fn run(self, sealer: SequencerSealer) {
        assert!(!self.actions.is_empty(), "Test scenario can't be empty");

        let batch_executor_base = TestBatchExecutorBuilder::new(&self);
        let (stop_sender, stop_receiver) = watch::channel(false);
        let (io, output_handler) = TestIO::new(stop_sender, self);
        let state_keeper = ZkSyncStateKeeper::new(
            stop_receiver,
            Box::new(io),
            Box::new(batch_executor_base),
            output_handler,
            Arc::new(sealer),
        );
        let sk_thread = tokio::spawn(state_keeper.run());

        // We must assume that *theoretically* state keeper may ignore the stop signal from IO once scenario is
        // completed, so we spawn it in a separate thread to not get test stuck.
        let hard_timeout = Duration::from_secs(60);
        let poll_interval = Duration::from_millis(50);
        let start = Instant::now();
        while start.elapsed() <= hard_timeout {
            if sk_thread.is_finished() {
                sk_thread
                    .await
                    .unwrap_or_else(|_| panic!("State keeper thread panicked"))
                    .unwrap();
                return;
            }
            tokio::time::sleep(poll_interval).await;
        }
        panic!("State keeper test did not exit until the hard timeout, probably it got stuck");
    }
}

/// Creates a random transaction. Provided tx number would be used as a transaction hash,
/// so it's easier to understand which transaction caused test to fail.
pub(crate) fn random_tx(tx_number: u64) -> Transaction {
    let mut tx = create_l2_transaction(10, 100);
    // Set the `tx_number` as tx hash so if transaction causes problems,
    // it'll be easier to understand which one.
    tx.set_input(H256::random().0.to_vec(), H256::from_low_u64_be(tx_number));
    tx.into()
}

/// Creates a random protocol upgrade transaction. Provided tx number would be used as a transaction hash,
/// so it's easier to understand which transaction caused test to fail.
pub(crate) fn random_upgrade_tx(tx_number: u64) -> ProtocolUpgradeTx {
    let mut tx = ProtocolUpgradeTx {
        execute: Default::default(),
        common_data: Default::default(),
        received_timestamp_ms: 0,
    };
    tx.common_data.canonical_tx_hash = H256::from_low_u64_be(tx_number);
    tx
}

/// Creates a `TxExecutionResult` object denoting a successful tx execution.
pub(crate) fn successful_exec() -> TxExecutionResult {
    TxExecutionResult::Success {
        tx_result: Box::new(VmExecutionResultAndLogs {
            result: ExecutionResult::Success { output: vec![] },
            logs: Default::default(),
            statistics: Default::default(),
            refunds: Default::default(),
        }),
        tx_metrics: Box::new(ExecutionMetricsForCriteria {
            l1_gas: Default::default(),
            execution_metrics: Default::default(),
        }),
        compressed_bytecodes: vec![],
        call_tracer_result: vec![],
        gas_remaining: Default::default(),
    }
}

/// Creates a `TxExecutionResult` object denoting a successful tx execution with the given execution metrics.
pub(crate) fn successful_exec_with_metrics(
    tx_metrics: ExecutionMetricsForCriteria,
) -> TxExecutionResult {
    TxExecutionResult::Success {
        tx_result: Box::new(VmExecutionResultAndLogs {
            result: ExecutionResult::Success { output: vec![] },
            logs: Default::default(),
            statistics: Default::default(),
            refunds: Default::default(),
        }),
        tx_metrics: Box::new(tx_metrics),
        compressed_bytecodes: vec![],
        call_tracer_result: vec![],
        gas_remaining: Default::default(),
    }
}

/// Creates a `TxExecutionResult` object denoting a tx that was rejected.
pub(crate) fn rejected_exec() -> TxExecutionResult {
    TxExecutionResult::RejectedByVm {
        reason: multivm::interface::Halt::InnerTxError,
    }
}

/// Creates a mock `PendingBatchData` object containing the provided sequence of miniblocks.
pub(crate) fn pending_batch_data(
    pending_miniblocks: Vec<MiniblockExecutionData>,
) -> PendingBatchData {
    PendingBatchData {
        l1_batch_env: default_l1_batch_env(1, 1, FEE_ACCOUNT),
        system_env: SystemEnv {
            zk_porter_available: false,
            version: ProtocolVersionId::latest(),
            base_system_smart_contracts: BASE_SYSTEM_CONTRACTS.clone(),
            gas_limit: BLOCK_GAS_LIMIT,
            execution_mode: TxExecutionMode::VerifyExecute,
            default_validation_computational_gas_limit: BLOCK_GAS_LIMIT,
            chain_id: L2ChainId::from(270),
        },
        pending_miniblocks,
    }
}

#[allow(clippy::type_complexity, clippy::large_enum_variant)] // It's OK for tests.
enum ScenarioItem {
    /// Configures scenario to repeatedly return `None` to tx requests until the next action from the scenario happens.
    NoTxsUntilNextAction(&'static str),
    /// Increments protocol version in IO state.
    IncrementProtocolVersion(&'static str),
    Tx(&'static str, Transaction, TxExecutionResult),
    Rollback(&'static str, Transaction),
    Reject(&'static str, Transaction, Option<String>),
    MiniblockSeal(
        &'static str,
        Option<Box<dyn FnOnce(&UpdatesManager) + Send>>,
    ),
    BatchSeal(
        &'static str,
        Option<Box<dyn FnOnce(&UpdatesManager) + Send>>,
    ),
}

impl fmt::Debug for ScenarioItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTxsUntilNextAction(descr) => formatter
                .debug_tuple("NoTxsUntilNextAction")
                .field(descr)
                .finish(),
            Self::IncrementProtocolVersion(descr) => formatter
                .debug_tuple("IncrementProtocolVersion")
                .field(descr)
                .finish(),
            Self::Tx(descr, tx, result) => formatter
                .debug_tuple("Tx")
                .field(descr)
                .field(tx)
                .field(result)
                .finish(),
            Self::Rollback(descr, tx) => formatter
                .debug_tuple("Rollback")
                .field(descr)
                .field(tx)
                .finish(),
            Self::Reject(descr, tx, err) => formatter
                .debug_tuple("Reject")
                .field(descr)
                .field(tx)
                .field(err)
                .finish(),
            Self::MiniblockSeal(descr, _) => {
                formatter.debug_tuple("MiniblockSeal").field(descr).finish()
            }
            Self::BatchSeal(descr, _) => formatter.debug_tuple("BatchSeal").field(descr).finish(),
        }
    }
}

type ExpectedTransactions = VecDeque<HashMap<H256, VecDeque<TxExecutionResult>>>;

#[derive(Debug, Default)]
pub(crate) struct TestBatchExecutorBuilder {
    /// Sequence of known transaction execution results per batch.
    /// We need to store txs for each batch separately, since the same transaction
    /// can be executed in several batches (e.g. after an `ExcludeAndSeal` rollback).
    /// When initializing each batch, we will `pop_front` known txs for the corresponding executor.
    txs: ExpectedTransactions,
    /// Set of transactions that would be rolled back at least once.
    rollback_set: HashSet<H256>,
}

impl TestBatchExecutorBuilder {
    pub(super) fn new(scenario: &TestScenario) -> Self {
        let mut txs = VecDeque::new();
        let mut batch_txs = HashMap::new();
        let mut rollback_set = HashSet::new();

        // Insert data about the pending batch, if it exists.
        // All the txs from the pending batch must succeed.
        if let Some(pending_batch) = &scenario.pending_batch {
            for tx in pending_batch
                .pending_miniblocks
                .iter()
                .flat_map(|miniblock| &miniblock.txs)
            {
                batch_txs.insert(tx.hash(), vec![successful_exec()].into());
            }
        }

        // Go through scenario and collect per-batch transactions and the overall rollback set.
        for item in &scenario.actions {
            match item {
                ScenarioItem::Tx(_, tx, result) => {
                    batch_txs
                        .entry(tx.hash())
                        .and_modify(|txs: &mut VecDeque<TxExecutionResult>| {
                            txs.push_back(result.clone())
                        })
                        .or_insert_with(|| {
                            let mut txs = VecDeque::with_capacity(1);
                            txs.push_back(result.clone());
                            txs
                        });
                }
                ScenarioItem::Rollback(_, tx) => {
                    rollback_set.insert(tx.hash());
                }
                ScenarioItem::Reject(_, tx, _) => {
                    rollback_set.insert(tx.hash());
                }
                ScenarioItem::BatchSeal(_, _) => txs.push_back(mem::take(&mut batch_txs)),
                _ => {}
            }
        }

        // Some batch seal may not be included into scenario, dump such txs if they exist.
        if !batch_txs.is_empty() {
            txs.push_back(mem::take(&mut batch_txs));
        }
        // After sealing the batch, state keeper initialized a new one, so we need to create an empty set
        // for the initialization of the "next-to-last" batch.
        txs.push_back(HashMap::default());

        Self { txs, rollback_set }
    }

    /// Adds successful transactions to be executed in a single L1 batch.
    pub(crate) fn push_successful_transactions(&mut self, tx_hashes: &[H256]) {
        let txs = tx_hashes
            .iter()
            .copied()
            .map(|tx_hash| (tx_hash, VecDeque::from([successful_exec()])));
        self.txs.push_back(txs.collect());
    }
}

#[async_trait]
impl BatchExecutor for TestBatchExecutorBuilder {
    async fn init_batch(
        &mut self,
        _l1batch_params: L1BatchEnv,
        _system_env: SystemEnv,
        _stop_receiver: &watch::Receiver<bool>,
    ) -> Option<BatchExecutorHandle> {
        let (commands_sender, commands_receiver) = mpsc::channel(1);

        let executor = TestBatchExecutor::new(
            commands_receiver,
            self.txs.pop_front().unwrap(),
            self.rollback_set.clone(),
        );
        let handle = tokio::task::spawn_blocking(move || executor.run());

        Some(BatchExecutorHandle::from_raw(handle, commands_sender))
    }
}

#[derive(Debug)]
pub(super) struct TestBatchExecutor {
    commands: mpsc::Receiver<Command>,
    /// Mapping tx -> response.
    /// The same transaction can be executed several times, so we use a sequence of responses and consume them by one.
    txs: HashMap<H256, VecDeque<TxExecutionResult>>,
    /// Set of transactions that are expected to be rolled back.
    rollback_set: HashSet<H256>,
    /// Last executed tx hash.
    last_tx: H256,
}

impl TestBatchExecutor {
    pub(super) fn new(
        commands: mpsc::Receiver<Command>,
        txs: HashMap<H256, VecDeque<TxExecutionResult>>,
        rollback_set: HashSet<H256>,
    ) -> Self {
        Self {
            commands,
            txs,
            rollback_set,
            last_tx: H256::default(), // We don't expect rollbacks until the first tx is executed.
        }
    }

    pub(super) fn run(mut self) {
        while let Some(cmd) = self.commands.blocking_recv() {
            match cmd {
                Command::ExecuteTx(tx, resp) => {
                    let result = self
                        .txs
                        .get_mut(&tx.hash())
                        .unwrap()
                        .pop_front()
                        .unwrap_or_else(|| {
                            panic!(
                                "Received a request to execute an unknown transaction: {:?}",
                                tx
                            )
                        });
                    resp.send(result).unwrap();
                    self.last_tx = tx.hash();
                }
                Command::StartNextMiniblock(_, resp) => {
                    resp.send(()).unwrap();
                }
                Command::RollbackLastTx(resp) => {
                    // This is an additional safety check: IO would check that every rollback is included in the
                    // test scenario, but here we want to additionally check that each such request goes to the
                    // the batch executor as well.
                    if !self.rollback_set.contains(&self.last_tx) {
                        // Request to rollback an unexpected tx.
                        panic!(
                            "Received a request to rollback an unexpected tx. Last executed tx: {:?}",
                            self.last_tx
                        )
                    }
                    resp.send(()).unwrap();
                    // It's OK to not update `last_executed_tx`, since state keeper never should rollback more than 1
                    // tx in a row, and it's going to cause a panic anyway.
                }
                Command::FinishBatch(resp) => {
                    // Blanket result, it doesn't really matter.
                    resp.send(default_vm_block_result()).unwrap();
                    return;
                }
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TestPersistence {
    actions: Arc<Mutex<VecDeque<ScenarioItem>>>,
    stop_sender: Arc<watch::Sender<bool>>,
}

impl TestPersistence {
    fn pop_next_item(&self, request: &str) -> ScenarioItem {
        let mut actions = self.actions.lock().expect("scenario queue is poisoned");
        let action = actions
            .pop_front()
            .unwrap_or_else(|| panic!("no action for request: {request}"));
        // If that was a last action, tell the state keeper to stop after that.
        if actions.is_empty() {
            self.stop_sender.send_replace(true);
        }
        action
    }
}

#[async_trait]
impl StateKeeperOutputHandler for TestPersistence {
    async fn handle_miniblock(&mut self, updates_manager: &UpdatesManager) -> anyhow::Result<()> {
        let action = self.pop_next_item("seal_miniblock");
        let ScenarioItem::MiniblockSeal(_, check_fn) = action else {
            anyhow::bail!("Unexpected action: {:?}", action);
        };
        if let Some(check_fn) = check_fn {
            check_fn(updates_manager);
        }
        Ok(())
    }

    async fn handle_l1_batch(&mut self, updates_manager: &UpdatesManager) -> anyhow::Result<()> {
        let action = self.pop_next_item("seal_l1_batch");
        let ScenarioItem::BatchSeal(_, check_fn) = action else {
            anyhow::bail!("Unexpected action: {:?}", action);
        };
        if let Some(check_fn) = check_fn {
            check_fn(updates_manager);
        }
        Ok(())
    }
}

pub(super) struct TestIO {
    stop_sender: Arc<watch::Sender<bool>>,
    batch_number: L1BatchNumber,
    timestamp: u64,
    fee_input: BatchFeeInput,
    miniblock_number: MiniblockNumber,
    fee_account: Address,
    pending_batch: Option<PendingBatchData>,
    l1_batch_seal_fn: Box<SealFn>,
    miniblock_seal_fn: Box<SealFn>,
    actions: Arc<Mutex<VecDeque<ScenarioItem>>>,
    /// Internal flag that is being set if scenario was configured to return `None` to all the transaction
    /// requests until some other action happens.
    skipping_txs: bool,
    protocol_version: ProtocolVersionId,
    previous_batch_protocol_version: ProtocolVersionId, // FIXME: not updated
    protocol_upgrade_txs: HashMap<ProtocolVersionId, ProtocolUpgradeTx>,
}

impl fmt::Debug for TestIO {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("TestIO").finish_non_exhaustive()
    }
}

impl TestIO {
    pub(super) fn new(
        stop_sender: watch::Sender<bool>,
        scenario: TestScenario,
    ) -> (Self, OutputHandler) {
        let stop_sender = Arc::new(stop_sender);
        let actions = Arc::new(Mutex::new(scenario.actions));
        let persistence = TestPersistence {
            stop_sender: stop_sender.clone(),
            actions: actions.clone(),
        };

        let (miniblock_number, timestamp) = if let Some(pending_batch) = &scenario.pending_batch {
            let last_pending_miniblock = pending_batch
                .pending_miniblocks
                .last()
                .expect("pending batch should have at least one miniblock");
            (
                last_pending_miniblock.number + 1,
                last_pending_miniblock.timestamp + 1,
            )
        } else {
            (MiniblockNumber(1), 1)
        };
        let this = Self {
            stop_sender,
            batch_number: L1BatchNumber(1),
            timestamp,
            fee_input: BatchFeeInput::default(),
            pending_batch: scenario.pending_batch,
            l1_batch_seal_fn: scenario.l1_batch_seal_fn,
            miniblock_seal_fn: scenario.miniblock_seal_fn,
            actions,
            miniblock_number,
            fee_account: FEE_ACCOUNT,
            skipping_txs: false,
            protocol_version: ProtocolVersionId::latest(),
            previous_batch_protocol_version: ProtocolVersionId::latest(),
            protocol_upgrade_txs: HashMap::default(),
        };
        (this, OutputHandler::new(Box::new(persistence)))
    }

    pub(super) fn add_upgrade_tx(&mut self, version: ProtocolVersionId, tx: ProtocolUpgradeTx) {
        self.protocol_upgrade_txs.insert(version, tx);
    }

    fn pop_next_item(&mut self, request: &str) -> ScenarioItem {
        let mut actions = self.actions.lock().expect("scenario queue is poisoned");
        loop {
            let action = actions.pop_front().unwrap_or_else(|| {
                panic!(
                    "Test scenario is empty, but the following action was done by the state keeper: {request}"
                );
            });
            // If that was a last action, tell the state keeper to stop after that.
            if actions.is_empty() {
                self.stop_sender.send_replace(true);
            }

            match &action {
                ScenarioItem::NoTxsUntilNextAction(_) => {
                    self.skipping_txs = true;
                    // This is a mock item, so pop an actual one for the IO to process.
                    continue;
                }
                ScenarioItem::IncrementProtocolVersion(_) => {
                    self.protocol_version = (self.protocol_version as u16 + 1)
                        .try_into()
                        .expect("Cannot increment latest version");
                    // This is a mock item, so pop an actual one for the IO to process.
                    continue;
                }
                _ => break action,
            }
        }
    }
}

impl IoSealCriteria for TestIO {
    fn should_seal_l1_batch_unconditionally(&mut self, manager: &UpdatesManager) -> bool {
        (self.l1_batch_seal_fn)(manager)
    }

    fn should_seal_miniblock(&mut self, manager: &UpdatesManager) -> bool {
        (self.miniblock_seal_fn)(manager)
    }
}

#[async_trait]
impl StateKeeperIO for TestIO {
    fn chain_id(&self) -> L2ChainId {
        L2ChainId::default()
    }

    async fn initialize(&mut self) -> anyhow::Result<(IoCursor, Option<PendingBatchData>)> {
        let cursor = IoCursor {
            next_miniblock: self.miniblock_number,
            prev_miniblock_hash: H256::zero(),
            prev_miniblock_timestamp: self.timestamp.saturating_sub(1),
            l1_batch: self.batch_number,
        };
        let pending_batch = self.pending_batch.take();
        Ok((cursor, pending_batch))
    }

    async fn wait_for_new_batch_params(
        &mut self,
        cursor: &IoCursor,
        _max_wait: Duration,
    ) -> anyhow::Result<Option<L1BatchParams>> {
        assert_eq!(cursor.next_miniblock, self.miniblock_number);
        assert_eq!(cursor.l1_batch, self.batch_number);

        let params = L1BatchParams {
            protocol_version: self.protocol_version,
            validation_computational_gas_limit: BLOCK_GAS_LIMIT,
            operator_address: self.fee_account,
            fee_input: self.fee_input,
            first_miniblock: MiniblockParams {
                timestamp: self.timestamp,
                virtual_blocks: 1,
            },
        };
        self.miniblock_number += 1;
        self.timestamp += 1;
        self.batch_number += 1;
        Ok(Some(params))
    }

    async fn wait_for_new_miniblock_params(
        &mut self,
        cursor: &IoCursor,
        _max_wait: Duration,
    ) -> anyhow::Result<Option<MiniblockParams>> {
        assert_eq!(cursor.next_miniblock, self.miniblock_number);
        let params = MiniblockParams {
            timestamp: self.timestamp,
            // 1 is just a constant used for tests.
            virtual_blocks: 1,
        };
        self.miniblock_number += 1;
        self.timestamp += 1;
        Ok(Some(params))
    }

    async fn wait_for_next_tx(
        &mut self,
        max_wait: Duration,
    ) -> anyhow::Result<Option<Transaction>> {
        let action = self.pop_next_item("wait_for_next_tx");

        // Check whether we should ignore tx requests.
        if self.skipping_txs {
            // As per expectation, we should provide a delay given by the state keeper.
            tokio::time::sleep(max_wait).await;
            // Return the action to the scenario as we don't use it.
            self.actions.lock().unwrap().push_front(action);
            return Ok(None);
        }

        // We shouldn't, process normally.
        let ScenarioItem::Tx(_, tx, _) = action else {
            panic!("Unexpected action: {:?}", action);
        };
        Ok(Some(tx))
    }

    async fn rollback(&mut self, tx: Transaction) -> anyhow::Result<()> {
        let action = self.pop_next_item("rollback");
        let ScenarioItem::Rollback(_, expected_tx) = action else {
            panic!("Unexpected action: {:?}", action);
        };
        assert_eq!(
            tx, expected_tx,
            "Incorrect transaction has been rolled back"
        );
        self.skipping_txs = false;
        Ok(())
    }

    async fn reject(&mut self, tx: &Transaction, error: &str) -> anyhow::Result<()> {
        let action = self.pop_next_item("reject");
        let ScenarioItem::Reject(_, expected_tx, expected_err) = action else {
            panic!("Unexpected action: {:?}", action);
        };
        assert_eq!(tx, &expected_tx, "Incorrect transaction has been rejected");
        if let Some(expected_err) = expected_err {
            assert!(
                error.contains(&expected_err),
                "Transaction was rejected with an unexpected error. Expected part was {}, but the actual error was {}",
                expected_err,
                error
            );
        }
        self.skipping_txs = false;
        Ok(())
    }

    async fn load_base_system_contracts(
        &mut self,
        _protocol_version: ProtocolVersionId,
        _cursor: &IoCursor,
    ) -> anyhow::Result<BaseSystemContracts> {
        Ok(BASE_SYSTEM_CONTRACTS.clone())
    }

    async fn load_batch_version_id(
        &mut self,
        _number: L1BatchNumber,
    ) -> anyhow::Result<ProtocolVersionId> {
        Ok(self.previous_batch_protocol_version)
    }

    async fn load_upgrade_tx(
        &mut self,
        version_id: ProtocolVersionId,
    ) -> anyhow::Result<Option<ProtocolUpgradeTx>> {
        Ok(self.protocol_upgrade_txs.get(&version_id).cloned())
    }

    async fn load_batch_state_hash(
        &mut self,
        _l1_batch_number: L1BatchNumber,
    ) -> anyhow::Result<H256> {
        Ok(H256::zero())
    }
}

/// `BatchExecutor` which doesn't check anything at all. Accepts all transactions.
// FIXME: move to `utils`?
#[derive(Debug)]
pub(crate) struct MockBatchExecutor;

#[async_trait]
impl BatchExecutor for MockBatchExecutor {
    async fn init_batch(
        &mut self,
        _l1batch_params: L1BatchEnv,
        _system_env: SystemEnv,
        _stop_receiver: &watch::Receiver<bool>,
    ) -> Option<BatchExecutorHandle> {
        let (send, recv) = mpsc::channel(1);
        let handle = tokio::task::spawn(async {
            let mut recv = recv;
            while let Some(cmd) = recv.recv().await {
                match cmd {
                    Command::ExecuteTx(_, resp) => resp.send(successful_exec()).unwrap(),
                    Command::StartNextMiniblock(_, resp) => resp.send(()).unwrap(),
                    Command::RollbackLastTx(_) => panic!("unexpected rollback"),
                    Command::FinishBatch(resp) => {
                        // Blanket result, it doesn't really matter.
                        resp.send(default_vm_block_result()).unwrap();
                        return;
                    }
                }
            }
        });
        Some(BatchExecutorHandle::from_raw(handle, send))
    }
}
