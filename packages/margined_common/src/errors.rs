use cosmwasm_std::{StdError, Uint128};
use cw_controllers::AdminError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Admin(#[from] AdminError),

    #[error("Vault is below minimum collateral amount (0.5 base denom)")]
    BelowMinCollateralAmount {},

    #[error("Strategy denom not initialised")]
    DenomNotInitialized {},

    #[error("Zero Division Error")]
    DivideByZero {},

    #[error("Expired")]
    Expired {},

    #[error("Event '{0}' not found")]
    EventNotFound(String),

    #[error("Invalid funds")]
    InvalidFunds {},

    #[error("Invalid liquidation")]
    InvalidLiquidation {},

    #[error("Invalid denom {0} not found")]
    InvalidDenom(String),

    #[error("Contract is already open")]
    IsOpen {},

    #[error("Invalid duration cannot be greater than {0}")]
    InvalidDuration(u64),

    #[error("Invalid ownership, new owner cannot be the same as existing")]
    InvalidOwnership {},

    #[error("Invalid reply id")]
    InvalidReplyId,

    #[error("Insufficient balance")]
    InsufficientBalance {},

    #[error("Insufficient denom {0}. {1} required")]
    InsufficientPower(String, Uint128),

    #[error("Non-payable entry point")]
    NonPayable {},

    #[error("Unpause delay not expired")]
    NotExpired {},

    #[error("Cannot perform action as contract is not open")]
    NotOpen {},

    #[error("Invalid denom {0} not found in pool {1}")]
    NotFoundInPool(String, String),

    #[error("Owner not set")]
    NoOwner {},

    #[error("Contract is not paused")]
    NotPaused {},

    #[error("Contract is not admin of the power token")]
    NotTokenAdmin {},

    #[error("Proposal not found")]
    ProposalNotFound {},

    #[error("Cannot perform action as contract is paused")]
    Paused {},

    #[error("Vault is safe, cannot be liquidated")]
    SafeVault {},

    #[error("Error in submessage: '{0}'")]
    SubMsgError(String),

    #[error("Strategy Cap Exceeded")]
    StrategyCapExceeded {},

    #[error("Token denom '{0}' is not supported")]
    TokenUnsupported(String),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Unknown reply-id: '{0}'")]
    UnknownReplyId(u64),

    #[error("Vault is not safe, cannot perform operation")]
    UnsafeVault {},

    #[error("Vault does not exist, cannot perform operation")]
    VaultDoesNotExist {},

    #[error("Zero mint not supported")]
    ZeroMint {},

    #[error("Zero transfer not supported")]
    ZeroTransfer {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}

impl ContractError {
    pub fn generic_err(msg: impl Into<String>) -> ContractError {
        ContractError::Std(StdError::generic_err(msg))
    }
}
