use bitcoin::hash_types::{BlockHash, TxMerkleNode};
use drivechain as drive;
use miette::{IntoDiagnostic as _, Result};
use std::collections::HashMap;
use std::str::FromStr;

// FIXME: Figure out how to pass std::vector<unsigned char> directly, without
// hex encoding.
#[cxx::bridge]
mod ffi {
    #[derive(Debug)]
    struct Block {
        data: String,
        time: i64,
        main_block_hash: String,
    }
    #[derive(Debug)]
    struct Output {
        address: String,
        amount: u64,
    }
    #[derive(Debug)]
    struct Withdrawal {
        outpoint: String,
        main_address: String,
        main_fee: u64,
        amount: u64,
    }
    #[derive(Debug)]
    struct Refund {
        outpoint: String,
        amount: u64,
    }
    #[derive(Debug)]
    enum BMMState {
        Succeded,
        Failed,
        Pending,
    }
    extern "Rust" {
        type Drivechain;
        fn new_drivechain(
            db_path: &str,
            this_sidechain: usize,
            main_host: &str,
            main_port: u16,
            rpcuser: &str,
            rpcpassword: &str,
        ) -> Result<Box<Drivechain>>;
        fn get_mainchain_tip(&self) -> Result<String>;
        fn get_prev_main_block_hash(&self, main_block_hash: &str) -> Result<Vec<u8>>;
        fn confirm_bmm(&mut self) -> Result<BMMState>;
        fn attempt_bmm(
            &mut self,
            critical_hash: &str,
            prev_main_block_hash: &str,
            amount: u64,
        ) -> Result<()>;
        fn connect_block(
            &mut self,
            deposits: Vec<Output>,
            withdrawals: Vec<Withdrawal>,
            refunds: Vec<Refund>,
            just_check: bool,
        ) -> Result<bool>;
        fn disconnect_block(
            &mut self,
            deposits: Vec<Output>,
            withdrawals: Vec<String>,
            refunds: Vec<String>,
            just_check: bool,
        ) -> Result<bool>;
        fn attempt_bundle_broadcast(&mut self) -> Result<()>;
        fn is_outpoint_spent(&self, outpoint: &str) -> Result<bool>;
        fn is_main_block_connected(&self, main_block_hash: &str) -> Result<bool>;
        fn verify_bmm(&self, main_block_hash: &str, critical_hash: &str) -> Result<bool>;
        fn get_deposit_outputs(&self) -> Result<Vec<Output>>;
        fn format_deposit_address(&self, address: &str) -> String;
        fn extract_mainchain_address_bytes(address: &str) -> Result<Vec<u8>>;
        fn get_new_mainchain_address(&self) -> Result<String>;
        fn create_deposit(&self, address: &str, amount: u64, fee: u64) -> Result<String>;
        fn generate(&self, n: u64) -> Result<Vec<String>>;
        fn flush(&mut self) -> Result<usize>;
    }
}

pub struct Drivechain(drive::Drivechain);

fn new_drivechain(
    db_path: &str,
    this_sidechain: usize,
    main_host: &str,
    main_port: u16,
    rpcuser: &str,
    rpcpassword: &str,
) -> Result<Box<Drivechain>> {
    let drivechain = drive::Drivechain::new(
        db_path,
        this_sidechain,
        main_host,
        main_port,
        rpcuser.into(),
        rpcpassword.into(),
    )
    .into_diagnostic()?;
    Ok(Box::new(Drivechain(drivechain)))
}

impl Drivechain {
    fn get_mainchain_tip(&self) -> Result<String> {
        let tip = self.0.get_mainchain_tip().into_diagnostic()?;
        Ok(tip.to_string())
    }
    fn get_prev_main_block_hash(&self, main_block_hash: &str) -> Result<Vec<u8>> {
        let main_block_hash = BlockHash::from_str(main_block_hash).into_diagnostic()?;
        let prev_hash = self
            .0
            .get_prev_main_block_hash(&main_block_hash)
            .into_diagnostic()?;
        Ok(prev_hash.to_vec())
    }
    fn confirm_bmm(&mut self) -> Result<ffi::BMMState> {
        self.0
            .confirm_bmm()
            .map(|state| match state {
                drivechain::BMMState::Succeded => ffi::BMMState::Succeded,
                drivechain::BMMState::Failed => ffi::BMMState::Failed,
                drivechain::BMMState::Pending => ffi::BMMState::Pending,
            })
            .into_diagnostic()
    }

    fn attempt_bmm(
        &mut self,
        critical_hash: &str,
        prev_main_block_hash: &str,
        amount: u64,
    ) -> Result<()> {
        let critical_hash = TxMerkleNode::from_str(critical_hash).into_diagnostic()?;
        let prev_main_block_hash = BlockHash::from_str(prev_main_block_hash).into_diagnostic()?;
        let amount = bitcoin::Amount::from_sat(amount);
        self.0
            .attempt_bmm(&critical_hash, &prev_main_block_hash, amount)
            .into_diagnostic()?;
        Ok(())
    }

    fn is_main_block_connected(&self, main_block_hash: &str) -> Result<bool> {
        let main_block_hash = BlockHash::from_str(main_block_hash).into_diagnostic()?;
        self.0
            .is_main_block_connected(&main_block_hash)
            .into_diagnostic()
    }

    fn verify_bmm(&self, main_block_hash: &str, critical_hash: &str) -> Result<bool> {
        let main_block_hash = BlockHash::from_str(main_block_hash).into_diagnostic()?;
        let critical_hash = TxMerkleNode::from_str(critical_hash).into_diagnostic()?;
        Ok(self.0.verify_bmm(&main_block_hash, &critical_hash).is_ok())
    }

    fn get_deposit_outputs(&self) -> Result<Vec<ffi::Output>> {
        Ok(self
            .0
            .get_deposit_outputs()
            .into_diagnostic()?
            .iter()
            .map(|output| ffi::Output {
                address: output.address.clone(),
                amount: output.amount,
            })
            .collect())
    }

    fn attempt_bundle_broadcast(&mut self) -> Result<()> {
        Ok(self.0.attempt_bundle_broadcast().into_diagnostic()?)
    }

    fn is_outpoint_spent(&self, outpoint: &str) -> Result<bool> {
        let outpoint = hex::decode(outpoint).into_diagnostic()?;
        self.0
            .is_outpoint_spent(outpoint.as_slice())
            .into_diagnostic()
    }

    fn connect_block(
        &mut self,
        deposits: Vec<ffi::Output>,
        withdrawals: Vec<ffi::Withdrawal>,
        refunds: Vec<ffi::Refund>,
        just_check: bool,
    ) -> Result<bool> {
        let deposits: Vec<drive::Deposit> = deposits
            .iter()
            .map(|output| drive::Deposit {
                address: output.address.clone(),
                amount: output.amount,
            })
            .collect();

        let withdrawals: Result<HashMap<Vec<u8>, drive::Withdrawal>> = withdrawals
            .into_iter()
            .map(|w| {
                let mut dest: [u8; 20] = Default::default();
                dest.copy_from_slice(hex::decode(w.main_address).into_diagnostic()?.as_slice());
                let mainchain_fee = w.main_fee;
                Ok((
                    hex::decode(w.outpoint).into_diagnostic()?,
                    drive::Withdrawal {
                        amount: w.amount,
                        dest,
                        mainchain_fee,
                        // height is set later in Db::connect_withdrawals.
                        height: 0,
                    },
                ))
            })
            .collect();

        let refunds: Result<HashMap<Vec<u8>, u64>> = refunds
            .iter()
            .map(|r| {
                Ok((
                    hex::decode(&r.outpoint).into_diagnostic()?.to_vec(),
                    r.amount,
                ))
            })
            .collect();
        Ok(self
            .0
            .connect_block(deposits.as_slice(), &withdrawals?, &refunds?, just_check)
            .is_ok())
    }

    fn disconnect_block(
        &mut self,
        deposits: Vec<ffi::Output>,
        withdrawals: Vec<String>,
        refunds: Vec<String>,
        just_check: bool,
    ) -> Result<bool> {
        let deposits: Vec<drive::Deposit> = deposits
            .iter()
            .map(|deposit| drive::Deposit {
                address: deposit.address.clone(),
                amount: deposit.amount,
            })
            .collect();
        let withdrawals: Result<Vec<Vec<u8>>> = withdrawals
            .iter()
            .map(|o| Ok(hex::decode(o).into_diagnostic()?.to_vec()))
            .collect();
        let refunds: Result<Vec<Vec<u8>>> = refunds
            .iter()
            .map(|r| Ok(hex::decode(r).into_diagnostic()?.to_vec()))
            .collect();
        Ok(self
            .0
            .disconnect_block(
                deposits.as_slice(),
                withdrawals?.as_slice(),
                refunds?.as_slice(),
                just_check,
            )
            .is_ok())
    }

    fn format_deposit_address(&self, address: &str) -> String {
        self.0.format_deposit_address(address)
    }

    fn get_new_mainchain_address(&self) -> Result<String> {
        let address = self.0.get_new_mainchain_address().into_diagnostic()?;
        Ok(address.to_string())
    }

    fn create_deposit(&self, address: &str, amount: u64, fee: u64) -> Result<String> {
        self.0
            .create_deposit(
                address,
                bitcoin::Amount::from_sat(amount),
                bitcoin::Amount::from_sat(fee),
            )
            .map(|txid| txid.to_string())
            .into_diagnostic()
    }

    fn generate(&self, n: u64) -> Result<Vec<String>> {
        self.0
            .generate(n as usize)
            .map(|hashes| hashes.iter().map(|hash| hash.to_string()).collect())
            .into_diagnostic()
    }

    fn flush(&mut self) -> Result<usize> {
        self.0.flush().into_diagnostic()
    }
}

fn extract_mainchain_address_bytes(address: &str) -> Result<Vec<u8>> {
    let address = bitcoin::Address::from_str(&address).into_diagnostic()?;
    let bytes = drive::Drivechain::extract_mainchain_address_bytes(&address).into_diagnostic()?;
    Ok(bytes.to_vec())
}
