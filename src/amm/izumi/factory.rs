abigen!(
    IiZiSwapFactory,
    r#"[
        event NewPool(address indexed tokenX,address indexed tokenY,uint24 indexed fee,uint24 pointDelta,address pool)
    ]"#;
);

pub const POOL_CREATED_EVENT_SIGNATURE: H256 = H256([
    240, 77, 166, 119, 85, 173, 245, 135, 57, 100, 158, 47, 185, 148, 154, 99, 40, 81, 129, 65,
    183, 172, 158, 68, 170, 16, 50, 6, 136, 176, 73, 0,
]);

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct iZiFactory {
    pub address: H160,
    pub creation_block: u64,
}

#[async_trait]
impl AutomatedMarketMakerFactory for iZiFactory {
    fn address(&self) -> H160 {
        self.address
    }

    fn creation_block(&self) -> u64 {
        self.creation_block
    }

    fn amm_created_event_signature(&self) -> H256 {
        POOL_CREATED_EVENT_SIGNATURE
    }

    async fn new_amm_from_log<M: 'static + Middleware>(
        &self,
        log: Log,
        middleware: Arc<M>,
    ) -> Result<AMM, DAMMError<M>> {
        todo!()
    }

    async fn get_all_amms<M: 'static + Middleware>(
        &self,
        to_block: Option<u64>,
        middleware: Arc<M>,
        step: u64,
    ) -> Result<Vec<AMM>, DAMMError<M>> {
        todo!()
    }

    async fn populate_amm_data<M: Middleware>(
        &self,
        amms: &mut [AMM],
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        todo!()
    }

    fn new_empty_amm_from_log(&self, log: Log) -> Result<AMM, ethers::abi::Error> {
        todo!()
    }
}