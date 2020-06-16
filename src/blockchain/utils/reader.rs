use std::io::{self};
use std::borrow::BorrowMut;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::errors::{OpError, OpErrorKind, OpResult};
use crate::blockchain::proto::varuint::VarUint;
use crate::blockchain::proto::block::Block;
use crate::blockchain::proto::header::BlockHeader;
use crate::blockchain::proto::tx::{Tx, TxOutpoint, TxInput, TxOutput};


/// Trait for structured reading of blockchain data
pub trait BlockchainRead: io::Read {
    fn read_256hash(&mut self) -> OpResult<[u8; 32]> {
        let mut arr = [0u8; 32];
        self.read_exact(arr.borrow_mut())?;
        Ok(arr)
    }

    fn read_u8_vec(&mut self, count: u32) -> OpResult<Vec<u8>> {
        let mut arr = vec![0u8; count as usize];
        self.read_exact(arr.borrow_mut())?;
        Ok(arr)
    }

    // Note: does not pop magic nor blocksize
    fn read_block(&mut self,
                  blk_index: u32,
                  blk_offset: usize,
                  blocksize: u32,
                  version_id: u8) -> OpResult<Block> {
        let header = self.read_block_header()?;
        let tx_count = VarUint::read_from(self)?;
        let txs = self.read_txs(tx_count.value, version_id)?;
        Ok(Block::new(blk_index, blk_offset, blocksize, header, tx_count, txs))
    }

    fn read_block_header(&mut self) -> OpResult<BlockHeader> {
        Ok(BlockHeader::new(
            self.read_u32::<LittleEndian>()?,
            self.read_256hash()?,
            self.read_256hash()?,
            self.read_u32::<LittleEndian>()?,
            self.read_u32::<LittleEndian>()?,
            self.read_u32::<LittleEndian>()?))
    }

    fn read_txs(&mut self, tx_count: u64, version_id: u8) -> OpResult<Vec<Tx>> {
        let mut txs: Vec<Tx> = Vec::with_capacity(tx_count as usize);
        for _ in 0..tx_count {
            let tx_version = self.read_u32::<LittleEndian>()?;
            let marker = self.read_u8()?;
            let in_count: VarUint;
            if marker == 0x00 {
                // SegWit hack
                /*let flag = */self.read_u8()?;
                in_count = VarUint::read_from(self)?;
            } else {
                in_count = match marker {
                    0x01..=0xfc => VarUint::from(marker),
                    0xfd => VarUint::from(self.read_u16::<LittleEndian>()?),
                    0xfe => VarUint::from(self.read_u32::<LittleEndian>()?),
                    0xff => VarUint::from(self.read_u64::<LittleEndian>()?),
                    _ => return Err(OpError::new(OpErrorKind::RuntimeError).join_msg("Invalid VarUint value")),
                };
            }
            let inputs = self.read_tx_inputs(in_count.value)?;
            let out_count = VarUint::read_from(self)?;
            let outputs = self.read_tx_outputs(out_count.value)?;
            if marker == 0x00 {
                // SegWit hack
                for _ in 0..in_count.value {
                    let item_count = VarUint::read_from(self)?;
                    for _ in 0..item_count.value {
                        let witness_len = VarUint::read_from(self)?;
                        let _ = self.read_u8_vec(witness_len.value as u32)?;
                    }
                }
            }
            let tx_locktime = self.read_u32::<LittleEndian>()?;
            let tx = Tx::new(tx_version,
                             in_count, &inputs,
                             out_count, &outputs,
                             tx_locktime,
                             version_id);
            txs.push(tx);
        }
        Ok(txs)
    }

    fn read_tx_outpoint(&mut self) -> OpResult<TxOutpoint> {
        let outpoint = TxOutpoint {
            txid: self.read_256hash()?,
            index: self.read_u32::<LittleEndian>()?
        };
        Ok(outpoint)
    }

    fn read_tx_inputs(&mut self, input_count: u64) -> OpResult<Vec<TxInput>> {
        let mut inputs: Vec<TxInput> = Vec::with_capacity(input_count as usize);
        for _ in 0..input_count {
            let outpoint = self.read_tx_outpoint()?;
            let script_len = VarUint::read_from(self)?;
            let script_sig = self.read_u8_vec(script_len.value as u32)?;
            let seq_no = self.read_u32::<LittleEndian>()?;

            let input = TxInput {
                outpoint,
                script_len,
                script_sig,
                seq_no,
            };
            inputs.push(input);
        }
        Ok(inputs)
    }

    fn read_tx_outputs(&mut self, output_count: u64) -> OpResult<Vec<TxOutput>> {
        let mut outputs: Vec<TxOutput> = Vec::with_capacity(output_count as usize);
        for _ in 0..output_count {
            let value = self.read_u64::<LittleEndian>()?;
            let script_len = VarUint::read_from(self)?;
            let script_pubkey = self.read_u8_vec(script_len.value as u32)?;

            let output = TxOutput {
                value,
                script_len,
                script_pubkey,
            };
            outputs.push(output);
        }
        Ok(outputs)
    }
}

/// All types that implement `Read` get methods defined in `BlockchainRead`
/// for free.
impl<R: io::Read + ?Sized> BlockchainRead for R {}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor};
    use byteorder::{LittleEndian, ReadBytesExt};
    use seek_bufread::BufReader;
    use crate::blockchain::utils::{arr_to_hex_swapped, arr_to_hex};
    use crate::blockchain::proto::script;
    use crate::blockchain::parser::types::{Coin, Bitcoin};

    #[test]
    fn test_bitcoin_parse_genesis_block() {
        // bitcoin genesis block as raw bytes
        let raw_data = vec![0xf9, 0xbe, 0xb4, 0xd9, 0x1d, 0x01, 0x00, 0x00,
                            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x3b, 0xa3, 0xed, 0xfd,
                            0x7a, 0x7b, 0x12, 0xb2, 0x7a, 0xc7, 0x2c, 0x3e,
                            0x67, 0x76, 0x8f, 0x61, 0x7f, 0xc8, 0x1b, 0xc3,
                            0x88, 0x8a, 0x51, 0x32, 0x3a, 0x9f, 0xb8, 0xaa,
                            0x4b, 0x1e, 0x5e, 0x4a, 0x29, 0xab, 0x5f, 0x49,
                            0xff, 0xff, 0x00, 0x1d, 0x1d, 0xac, 0x2b, 0x7c,
                            0x01, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff,
                            0xff, 0xff, 0x4d, 0x04, 0xff, 0xff, 0x00, 0x1d,
                            0x01, 0x04, 0x45, 0x54, 0x68, 0x65, 0x20, 0x54,
                            0x69, 0x6d, 0x65, 0x73, 0x20, 0x30, 0x33, 0x2f,
                            0x4a, 0x61, 0x6e, 0x2f, 0x32, 0x30, 0x30, 0x39,
                            0x20, 0x43, 0x68, 0x61, 0x6e, 0x63, 0x65, 0x6c,
                            0x6c, 0x6f, 0x72, 0x20, 0x6f, 0x6e, 0x20, 0x62,
                            0x72, 0x69, 0x6e, 0x6b, 0x20, 0x6f, 0x66, 0x20,
                            0x73, 0x65, 0x63, 0x6f, 0x6e, 0x64, 0x20, 0x62,
                            0x61, 0x69, 0x6c, 0x6f, 0x75, 0x74, 0x20, 0x66,
                            0x6f, 0x72, 0x20, 0x62, 0x61, 0x6e, 0x6b, 0x73,
                            0xff, 0xff, 0xff, 0xff, 0x01, 0x00, 0xf2, 0x05,
                            0x2a, 0x01, 0x00, 0x00, 0x00, 0x43, 0x41, 0x04,
                            0x67, 0x8a, 0xfd, 0xb0, 0xfe, 0x55, 0x48, 0x27,
                            0x19, 0x67, 0xf1, 0xa6, 0x71, 0x30, 0xb7, 0x10,
                            0x5c, 0xd6, 0xa8, 0x28, 0xe0, 0x39, 0x09, 0xa6,
                            0x79, 0x62, 0xe0, 0xea, 0x1f, 0x61, 0xde, 0xb6,
                            0x49, 0xf6, 0xbc, 0x3f, 0x4c, 0xef, 0x38, 0xc4,
                            0xf3, 0x55, 0x04, 0xe5, 0x1e, 0xc1, 0x12, 0xde,
                            0x5c, 0x38, 0x4d, 0xf7, 0xba, 0x0b, 0x8d, 0x57,
                            0x8a, 0x4c, 0x70, 0x2b, 0x6b, 0xf1, 0x1d, 0x5f,
                            0xac, 0x00, 0x00, 0x00, 0x00, 0xf9, 0xbe, 0xb4,
                            0xd9, 0xd7, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
                            0x00, 0x6f, 0xe2, 0x8c, 0x0a, 0xb6, 0xf1, 0xb3,
                            0x72, 0xc1, 0xa6, 0xa2, 0x46, 0xae, 0x63, 0xf7,
                            0x4f, 0x93, 0x1e, 0x83, 0x65, 0xe1, 0x5a, 0x08,
                            0x9c, 0x68, 0xd6, 0x19, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x98, 0x20, 0x51, 0xfd, 0x1e, 0x4b, 0xa7,
                            0x44, 0xbb, 0xbe, 0x68, 0x0e, 0x1f, 0xee, 0x14,
                            0x67, 0x7b, 0xa1, 0xa3, 0xc3, 0x54, 0x0b, 0xf7,
                            0xb1, 0xcd, 0xb6, 0x06, 0xe8, 0x57, 0x23, 0x3e,
                            0x0e, 0x61, 0xbc, 0x66, 0x49, 0xff, 0xff, 0x00,
                            0x1d, 0x01, 0xe3, 0x62, 0x99, 0x01, 0x01, 0x00,
                            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                            0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x07,
                            0x04, 0xff, 0xff, 0x00, 0x1d, 0x01, 0x04, 0xff,
                            0xff, 0xff, 0xff, 0x01, 0x00, 0xf2, 0x05, 0x2a,
                            0x01, 0x00, 0x00, 0x00, 0x43, 0x41, 0x04, 0x96,
                            0xb5, 0x38, 0xe8, 0x53, 0x51, 0x9c, 0x72, 0x6a,
                            0x2c, 0x91, 0xe6, 0x1e, 0xc1, 0x16, 0x00, 0xae,
                            0x13, 0x90, 0x81, 0x3a, 0x62, 0x7c, 0x66, 0xfb,
                            0x8b, 0xe7, 0x94, 0x7b, 0xe6, 0x3c, 0x52, 0xda,
                            0x75, 0x89, 0x37, 0x95, 0x15, 0xd4, 0xe0, 0xa6,
                            0x04, 0xf8, 0x14, 0x17, 0x81, 0xe6, 0x22, 0x94,
                            0x72, 0x11, 0x66, 0xbf, 0x62, 0x1e, 0x73, 0xa8,
                            0x2c, 0xbf, 0x23, 0x42, 0xc8, 0x58, 0xee, 0xac,
                            0x00, 0x00, 0x00, 0x0];
        let inner = Cursor::new(raw_data);
        let mut reader = BufReader::with_capacity(200, inner);

        let blk_id = 0;
        let blk_offset = 9;
        let magic: u32 = reader.read_u32::<LittleEndian>().unwrap();
        let blocksize: u32 = reader.read_u32::<LittleEndian>().unwrap();

        // Parse block
        let block = reader.read_block(blk_id, blk_offset, blocksize, Bitcoin.version_id()).unwrap();

        // Block Metadata
        assert_eq!(0xd9b4bef9,  magic);
        assert_eq!(285,         block.blocksize);

        // Block Header
        assert_eq!(0x00000001,  block.header.value.version);
        assert_eq!("0000000000000000000000000000000000000000000000000000000000000000",
                                arr_to_hex(&block.header.value.prev_hash));
        assert_eq!("3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a",
                                arr_to_hex(&block.header.value.merkle_root));
        assert_eq!("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
                                arr_to_hex_swapped(&block.header.hash));

        // Check against computed merkle root
        //assert_eq!(&block.header.merkle_root, &block.compute_merkle_root());
        assert_eq!(1231006505,  block.header.value.timestamp);
        assert_eq!(0x1d00ffff,  block.header.value.bits);
        assert_eq!(2083236893,  block.header.value.nonce);

        // Tx
        assert_eq!(0x01,        block.tx_count.value);
        assert_eq!(0x00000001,  block.txs[0].value.tx_version);

        // Tx Inputs
        assert_eq!(0x01,        block.txs[0].value.in_count.value);
        assert_eq!("0000000000000000000000000000000000000000000000000000000000000000",
                                arr_to_hex_swapped(&block.txs[0].value.inputs[0].outpoint.txid));
        assert_eq!(0xffffffff,  block.txs[0].value.inputs[0].outpoint.index);
        assert_eq!(0x4d,        block.txs[0].value.inputs[0].script_len.value);
        assert_eq!("04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73",
                                arr_to_hex(&block.txs[0].value.inputs[0].script_sig));
        assert_eq!(0xffffffff,  block.txs[0].value.inputs[0].seq_no);

        // Tx Outputs
        assert_eq!(0x01,        block.txs[0].value.out_count.value);
        assert_eq!(u64::from_be(0x00f2052a01000000),
                                block.txs[0].value.outputs[0].out.value);
        assert_eq!(0x43,        block.txs[0].value.outputs[0].out.script_len.value);

        let script_pubkey = &block.txs[0].value.outputs[0].out.script_pubkey;
        assert_eq!("4104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac",
                                arr_to_hex(&script_pubkey));
        assert_eq!(0x00000000,  block.txs[0].value.tx_locktime);

        assert_eq!("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", script::eval_from_bytes(script_pubkey, Bitcoin.version_id()).address);

                   /******* Genesis block raw data for reference (Most fields are little endian) *******
version            0x01000000   big endian??
prev_hash          0x0000000000000000000000000000000000000000000000000000000000000000
merkle_root        0x3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a
timestamp          0x29ab5f49
bits               0x1d00ffff
nonce              0x1dac2b7c
tx_count           0x01
tx_version         0x01000000   big endian??
tx.in_count        0x01
tx.in.prev_hash    0x0000000000000000000000000000000000000000000000000000000000000000
tx.in.out_id       0xffffffff
tx.in.script_len   0x4d
tx.in.script_sig   0x04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73
tx.in.sequence     0xffffffff
tx.out_count       0x01
tx.out.value       0x00f2052a01000000   big endian??
tx.out.script_len  0x43
tx.out.script_pubkey      0x4104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac
tx.lock_time       0x00000000
                   *********************************************************************************************************/
    }
}
