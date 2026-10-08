//! Pure pricing math for partial fills, kept separate for property testing.

/// Token B owed for `token_a_amount` at the offer's fixed price.
///
/// Partial fills round up so the maker never sells below their price. The
/// final fill charges whatever is left of `token_b_wanted`, so the maker
/// receives at least `token_b_wanted` in total.
pub fn token_b_for(
    token_a_amount: u64,
    token_a_remaining: u64,
    token_a_offered: u64,
    token_b_wanted: u64,
    token_b_received: u64,
) -> Option<u64> {
    if token_a_amount == token_a_remaining {
        return Some(token_b_wanted.saturating_sub(token_b_received));
    }
    let numerator = (token_a_amount as u128).checked_mul(token_b_wanted as u128)?;
    let amount = numerator.div_ceil(token_a_offered as u128);
    u64::try_from(amount).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn full_fill_pays_the_asking_price() {
        assert_eq!(token_b_for(100, 100, 100, 250, 0), Some(250));
    }

    #[test]
    fn partial_fill_is_pro_rata() {
        assert_eq!(token_b_for(40, 100, 100, 250, 0), Some(100));
    }

    #[test]
    fn partial_fill_rounds_up_for_the_maker() {
        // 1/3 of 10 = 3.33 -> 4
        assert_eq!(token_b_for(1, 3, 3, 10, 0), Some(4));
    }

    #[test]
    fn last_fill_settles_the_remainder() {
        // after paying 4 + 4 for two thirds, the last third costs 2
        assert_eq!(token_b_for(1, 1, 3, 10, 8), Some(2));
    }

    #[test]
    fn handles_large_amounts() {
        assert_eq!(
            token_b_for(u64::MAX / 2, u64::MAX, u64::MAX, u64::MAX, 0),
            Some(u64::MAX / 2)
        );
        // (MAX - 1) / 3 does not divide evenly, so it rounds up
        assert_eq!(
            token_b_for(1, 3, 3, u64::MAX - 1, 0),
            Some((u64::MAX - 1) / 3 + 1)
        );
    }

    proptest! {
        /// However an offer is split into fills, the maker gets at least
        /// their asking price and the taker never pays more than ceil(pro rata) per fill.
        #[test]
        fn maker_always_receives_the_full_price(
            offered in 1u64..1_000_000_000_000,
            wanted in 1u64..1_000_000_000_000,
            cuts in proptest::collection::vec(1u64..1_000, 1..20),
        ) {
            let mut remaining = offered;
            let mut received: u64 = 0;
            for cut in cuts {
                if remaining == 0 { break; }
                let amount = (offered / 1_000 * cut).clamp(1, remaining);
                let paid = token_b_for(amount, remaining, offered, wanted, received).unwrap();
                if amount < remaining {
                    let fair = (amount as u128 * wanted as u128).div_ceil(offered as u128) as u64;
                    prop_assert_eq!(paid, fair);
                }
                received += paid;
                remaining -= amount;
            }
            if remaining > 0 {
                received += token_b_for(remaining, remaining, offered, wanted, received).unwrap();
            }
            prop_assert!(received >= wanted);
        }
    }
}
