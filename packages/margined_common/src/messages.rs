use osmosis_std::types::{
    cosmos::base::v1beta1::Coin,
    osmosis::poolmanager::v1beta1::{
        MsgSwapExactAmountIn, MsgSwapExactAmountOut, SwapAmountInRoute, SwapAmountOutRoute,
    },
};

pub fn create_swap_message(
    sender: String,
    pool_id: u64,
    token_in_denom: String,
    token_out_denom: String,
    amount: String,
) -> MsgSwapExactAmountIn {
    MsgSwapExactAmountIn {
        sender,
        routes: vec![SwapAmountInRoute {
            pool_id,
            token_out_denom,
        }],
        token_in: Some(Coin {
            denom: token_in_denom,
            amount,
        }),
        token_out_min_amount: "1".to_string(),
    }
}

pub fn create_swap_exact_amount_in_message(
    sender: String,
    pool_id: u64,
    token_in_denom: String,
    token_out_denom: String,
    amount: String,
) -> MsgSwapExactAmountIn {
    MsgSwapExactAmountIn {
        sender,
        routes: vec![SwapAmountInRoute {
            pool_id,
            token_out_denom,
        }],
        token_in: Some(Coin {
            denom: token_in_denom,
            amount,
        }),
        token_out_min_amount: "1".to_string(),
    }
}

pub fn create_swap_exact_amount_out_message(
    sender: String,
    pool_id: u64,
    token_in_denom: String,
    token_out_denom: String,
    amount_out: String,
    token_in_max_amount: String,
) -> MsgSwapExactAmountOut {
    MsgSwapExactAmountOut {
        sender,
        routes: vec![SwapAmountOutRoute {
            pool_id,
            token_in_denom,
        }],
        token_out: Some(Coin {
            denom: token_out_denom,
            amount: amount_out,
        }),
        token_in_max_amount,
    }
}
