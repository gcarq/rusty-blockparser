
use blockchain::proto::block::Block;

use callbacks::Callback;
use errors::OpResult;

#[derive(Default)]
pub struct SimpleStats {
    total_volume: u64,
    n_valid_blocks: u64,
    n_transactions: u64,
    n_tx_inputs: u64,
    n_tx_outputs: u64,
}

impl Callback for SimpleStats {

    fn parse_args(_: Vec<String>) -> OpResult<Self> where Self: Sized {
        // We have no args, just return a new instance
        Ok(Default::default())
    }

    fn on_start(&mut self, _: usize) {
        info!(target: "callback", "Executing SimpleStats ...");
    }

    fn on_block(&mut self, block: Block, _: usize) {

        self.n_valid_blocks += 1;
        self.n_transactions += block.tx_count.value;

        self.n_tx_inputs += block.txs.iter()
            .map(|tx| &tx.value.in_count.value).fold(0, |sum, &val| sum + val);

        self.n_tx_outputs += block.txs.iter()
            .map(|tx| &tx.value.out_count.value).fold(0, |sum, &val| sum + val);

        self.total_volume += block.txs.iter().map(|tx| {
            tx.value.outputs.iter().map(|o| &o.out.value)
                .fold(0, |sum, &val| sum + val)
            }).fold(0, |sum, val| sum + val);
    }

    fn on_complete(&mut self, _: usize) {
        info!(target: "callback", "SimpleStats:");
        info!(target: "callback", "   -> valid blocks: {}", self.n_valid_blocks);
        info!(target: "callback", "   -> total transactions: {}", self.n_transactions);
        info!(target: "callback", "   -> total volume: {} ({} units)", self.total_volume as f64 * 1E-8, self.total_volume);
        info!(target: "callback", "   -> total tx inputs: {}", self.n_tx_inputs);
        info!(target: "callback", "   -> total tx_outputs: {}", self.n_tx_outputs);

        info!(target: "callback", "\n");
        info!(target: "callback", "   -> avg transactions per block: {:.2}",
            self.n_transactions.checked_div(self.n_valid_blocks).unwrap_or_default());
        info!(target: "callback", "   -> avg inputs per tx: {:.2}",
            self.n_tx_inputs.checked_div(self.n_transactions).unwrap_or_default());
        info!(target: "callback", "   -> avg outputs per tx: {:.2}",
            self.n_tx_outputs.checked_div(self.n_transactions).unwrap_or_default());
        info!(target: "callback", "   -> avg value per output: {:.2}\n",
            self.total_volume.checked_div(self.n_tx_outputs).unwrap_or_default() as f64 * 1E-8);
    }
}
