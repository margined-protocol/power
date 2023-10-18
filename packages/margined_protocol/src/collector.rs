use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    AddToken {
        token: String,
    },
    RemoveToken {
        token: String,
    },
    UpdateWhitelist {
        address: String,
    },
    SendToken {
        token: String,
        amount: Uint128,
        recipient: String,
    },
    ProposeNewOwner {
        new_owner: String,
        duration: u64,
    },
    RejectOwner {},
    ClaimOwnership {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Addr)]
    Owner {},
    #[returns(WhitelistResponse)]
    GetWhitelist {},
    #[returns(TokenResponse)]
    IsToken { token: String },
    #[returns(TokenLengthResponse)]
    GetTokenLength {},
    #[returns(AllTokenResponse)]
    GetTokenList { limit: Option<u32> },
    #[returns(OwnerProposalResponse)]
    GetOwnershipProposal {},
}

#[cw_serde]
pub struct WhitelistResponse {
    pub address: Option<Addr>,
}

#[cw_serde]
pub struct TokenResponse {
    pub is_token: bool,
}

#[cw_serde]
pub struct AllTokenResponse {
    pub token_list: Vec<String>,
}

#[cw_serde]
pub struct TokenLengthResponse {
    pub length: usize,
}

#[cw_serde]
pub struct OwnerProposalResponse {
    pub owner: Addr,
    pub expiry: u64,
}
