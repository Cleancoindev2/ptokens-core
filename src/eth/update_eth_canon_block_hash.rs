use crate::{
    types::Result,
    traits::DatabaseInterface,
    eth::{
        eth_state::EthState,
        eth_types::EthBlockAndReceipts,
        eth_database_utils::{
            get_eth_canon_block_from_db,
            get_eth_latest_block_from_db,
            put_eth_canon_block_hash_in_db,
            get_eth_canon_to_tip_length_from_db,
            maybe_get_nth_ancestor_eth_block_and_receipts,
        },
    },
};

fn does_canon_block_require_updating<D>(
    db: &D,
    calculated_canon_block_and_receipts: &EthBlockAndReceipts,
) -> Result<bool>
    where D: DatabaseInterface
{
    get_eth_canon_block_from_db(db)
        .map(|db_canon_block_and_receipts|
            db_canon_block_and_receipts.block.number <=
            calculated_canon_block_and_receipts.block.number - 1
        )
}

fn maybe_get_nth_ancestor_of_latest_block<D>(
    db: &D,
    n: &u64,
) -> Option<EthBlockAndReceipts>
    where D: DatabaseInterface
{
    info!("✔ Maybe getting ancestor #{} of latest ETH block...", n);
    match get_eth_latest_block_from_db(db) {
        Err(_) => None,
        Ok(block_and_receipts) => {
            maybe_get_nth_ancestor_eth_block_and_receipts(
                db,
                &block_and_receipts.block.hash,
                n
            )
        }
    }
}

fn maybe_update_canon_block_hash<D>(
    db: &D,
    canon_to_tip_length: &u64,
) -> Result<()>
    where D: DatabaseInterface
{
    match maybe_get_nth_ancestor_of_latest_block(db, canon_to_tip_length) {
        None => {
            info!("✔ No {}th ancestor block in db yet!", canon_to_tip_length);
            Ok(())
        }
        Some(ancestor_block) => {
            info!("✔ {}th ancestor block found...", canon_to_tip_length);
            match does_canon_block_require_updating(db, &ancestor_block)? {
                true => {
                    info!("✔ Updating canon block...");
                    put_eth_canon_block_hash_in_db(
                        db,
                        &ancestor_block.block.hash
                    )
                }
                false => {
                    info!("✔ Canon block does not require updating");
                    Ok(())
                }
            }
        }
    }
}

pub fn maybe_update_eth_canon_block_hash<D>(
    state: EthState<D>
) -> Result<EthState<D>>
    where D: DatabaseInterface
{
    info!("✔ Maybe updating ETH canon block hash...");
    get_eth_canon_to_tip_length_from_db(&state.db)
        .and_then(|canon_to_tip_length|
            maybe_update_canon_block_hash(
                &state.db,
                &canon_to_tip_length
            )
        )
        .map(|_| state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_utils::get_test_database,
        eth::{
            eth_test_utils::get_sequential_eth_blocks_and_receipts,
            eth_database_utils::{
                put_eth_canon_block_in_db,
                put_eth_latest_block_in_db,
                get_eth_canon_block_hash_from_db,
                put_eth_block_and_receipts_in_db,
            },
        },
    };

    #[test]
    fn should_return_true_if_canon_block_requires_updating() {
        let db = get_test_database();
        let blocks_and_receipts = get_sequential_eth_blocks_and_receipts();
        let canon_block = blocks_and_receipts[0].clone();
        let calculated_canon_block = blocks_and_receipts[1].clone();
        put_eth_canon_block_in_db(&db, &canon_block)
            .unwrap();
        let result = does_canon_block_require_updating(
            &db,
            &calculated_canon_block
        ).unwrap();
        assert!(result);
    }

    #[test]
    fn should_return_false_if_canon_block_does_not_require_updating() {
        let db = get_test_database();
        let blocks_and_receipts = get_sequential_eth_blocks_and_receipts();
        let canon_block = blocks_and_receipts[0].clone();
        let calculated_canon_block = blocks_and_receipts[0].clone();
        put_eth_canon_block_in_db(&db, &canon_block)
            .unwrap();
        let result = does_canon_block_require_updating(
            &db,
            &calculated_canon_block
        ).unwrap();
        assert!(!result);
    }

    #[test]
    fn should_return_block_if_nth_ancestor_of_latest_block_exists() {
        let db = get_test_database();
        let blocks_and_receipts = get_sequential_eth_blocks_and_receipts();
        let block_1 = blocks_and_receipts[0].clone();
        let block_2 = blocks_and_receipts[1].clone();
        put_eth_block_and_receipts_in_db(&db, &block_1)
            .unwrap();
        put_eth_latest_block_in_db(&db, &block_2)
            .unwrap();
        let result = maybe_get_nth_ancestor_of_latest_block(&db, &1);
        assert!(result == Some(block_1));
    }

    #[test]
    fn should_return_none_if_nth_ancestor_of_latest_block_does_not_exist() {
        let db = get_test_database();
        let blocks_and_receipts = get_sequential_eth_blocks_and_receipts();
        let block_1 = blocks_and_receipts[0].clone();
        put_eth_latest_block_in_db(&db, &block_1)
            .unwrap();
        let result = maybe_get_nth_ancestor_of_latest_block(&db, &1);
        assert!(result == None);
    }

    #[test]
    fn should_maybe_update_canon_block_hash() {
        let db = get_test_database();
        let blocks_and_receipts = get_sequential_eth_blocks_and_receipts();
        let canon_block = blocks_and_receipts[0].clone();
        let block_1 = blocks_and_receipts[1].clone();
        let latest_block = blocks_and_receipts[2].clone();
        let expected_canon_block_hash = block_1.block.hash;
        let canon_block_hash_before = canon_block.block.hash;
        put_eth_canon_block_in_db(&db, &canon_block)
            .unwrap();
        put_eth_block_and_receipts_in_db(&db, &block_1)
            .unwrap();
        put_eth_latest_block_in_db(&db, &latest_block)
            .unwrap();
        maybe_update_canon_block_hash(&db, &1).unwrap();
        let canon_block_hash_after = get_eth_canon_block_hash_from_db(&db)
            .unwrap();
        assert!(canon_block_hash_before != canon_block_hash_after);
        assert!(canon_block_hash_after == expected_canon_block_hash);
    }

    #[test]
    fn should_not_maybe_update_canon_block_hash() {
        let db = get_test_database();
        let blocks_and_receipts = get_sequential_eth_blocks_and_receipts();
        let canon_block = blocks_and_receipts[0].clone();
        let latest_block = blocks_and_receipts[1].clone();
        let canon_block_hash_before = canon_block.block.hash;
        put_eth_canon_block_in_db(&db, &canon_block)
            .unwrap();
        put_eth_latest_block_in_db(&db, &latest_block)
            .unwrap();
        maybe_update_canon_block_hash(&db, &1).unwrap();
        let canon_block_hash_after = get_eth_canon_block_hash_from_db(&db)
            .unwrap();
        assert!(canon_block_hash_before == canon_block_hash_after);
    }
}
