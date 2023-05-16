pub mod batch_request;
pub mod factory;

pub const SWAP_EVENT_SIGNATURE: H256 = H256([
    231, 119, 154, 54, 162, 138, 224, 228, 155, 203, 217, 252, 245, 114, 134, 251, 96, 118, 153,
    192, 195, 57, 194, 2, 233, 36, 149, 100, 5, 5, 97, 62,
]);
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct iZiSwapPool {
    pub address: H160,
    pub token_a: H160,
    pub token_a_decimals: u8,
    pub token_b: H160,
    pub token_b_decimals: u8,
    pub liquidity: u128,
    pub liquidity_x: u128,
    pub liquidity_y: u128,
    pub sqrt_price: U256,
    pub fee: u32,
    pub current_point: i32,
    pub point_delta: i32,
}
#[async_trait]
impl AutomatedMarketMaker for iZiPool {
    fn address(&self) -> H160 {
        self.address
    }
    async fn sync<M: Middleware>(&mut self, middleware: Arc<M>) -> Result<(), DAMMError<M>> {
        batch_request::sync_izi_pool_batch_request(self, middleware.clone()).await?;
    }
    fn sync_on_event_signatures(&self) -> Vec<H256> {
        vec![SWAP_EVENT_SIGNATURE]
    }
    fn tokens(&self) -> Vec<H160> {
        vec![self.token_a, self.token_b]
    }
    fn calculate_price(&self, base_token: H160) -> Result<f64, ArithmeticError> {
        let shift = self.token_a_decimals as i8 - self.token_b_decimals as i8;
        let price = match shift.cmp(&0) {
            Ordering::Less => 1.0001_f64.powi(self.current_point) / 10_f64.powi(-shift as i32),
            Ordering::Greater => 1.0001_f64.powi(self.current_point) * 10_f64.powi(shift as i32),
            Ordering::Equal => 1.0001_f64.powi(self.current_point),
        };
        if base_token == self.token_a {
            Ok(price)
        } else {
            Ok(1.0 / price)
        }
    }
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError> {
        let event_signature = log.topics[0];

        if event_signature == SWAP_EVENT_SIGNATURE {
            self.sync_from_swap_log(log)?;
        } else {
            Err(EventLogError::InvalidEventSignature)?
        }

        Ok(())
    }
    async fn populate_data<M: Middleware>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), DAMMError<M>> {
        batch_request::get_izi_pool_data_batch_request(self, block_number, middleware.clone())
            .await?;
        Ok(())
    }

    fn get_token_out(&self, token_in: H160) -> H160 {
        if self.token_a == token_in {
            self.token_b
        } else {
            self.token_a
        }
    }
}
