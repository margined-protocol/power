use crate::{
    state::{Config, CONFIG, DEFAULT_LIMIT, OWNER, STAKE_ASSETS, STATE},
    vault::{Vault, VAULTS},
};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    ensure, Addr, Decimal, DepsMut, Env, Event, MessageInfo, Order, Response, Uint128,
};
use cw_storage_plus::{Bound, Index, IndexList, IndexedMap, Item, MultiIndex};
use cw_utils::nonpayable;
use margined_common::errors::ContractError;
use margined_protocol::power::{Asset, Pool, StakeAsset, VaultType};

pub const SQOSMO: &str = "osmo1rk4hregdr63rlqqj0k2rjzk6kz7w6v6tw8f5fqx2wg8203eam5equ67tdl";
pub const SQATOM: &str = "osmo1zttzenjrnfr8tgrsfyu8kw0eshd8mas7yky43jjtactkhvmtkg2qz769y2";
pub const SQTIA: &str = "osmo18pfsg9n2kn6epty7uhur7vxfszadvflx6f66569ejc469k8p64pqrve3yz";

#[cw_serde]
pub struct OldConfig {
    pub query_contract: Addr,
    pub fee_pool_contract: Addr,
    pub fee_rate: Decimal,
    pub base_asset: Asset,
    pub power_asset: Asset,
    pub stake_asset: Option<Asset>,
    pub base_pool: Pool,
    pub power_pool: Pool,
    pub stake_pool: Option<Pool>,
    pub funding_period: u64,
    pub index_scale: u64,
    pub min_collateral_amount: Decimal,
}

#[cw_serde]
pub struct OldPool {
    pub id: u64,
    pub quote_denom: String,
}

#[cw_serde]
pub enum OldVaultType {
    Default,
    Staked,
}

pub const OLDCONFIG: Item<OldConfig> = Item::new("config");
pub const OLDVAULTS: IndexedMap<u64, OldVault, VaultIndexes> =
    IndexedMap::new("vaults", OLDINDEXES);

pub const OLDINDEXES: VaultIndexes<'_> = VaultIndexes {
    owner: MultiIndex::new(vault_operator_idx, "vaults", "vault__owner"),
};

#[cw_serde]
pub struct OldVault {
    pub operator: Addr,
    pub collateral: Uint128,
    pub short_amount: Uint128,
    pub vault_type: OldVaultType,
}

pub struct VaultIndexes<'a> {
    pub owner: MultiIndex<'a, Addr, OldVault, u64>,
}

impl<'a> IndexList<OldVault> for VaultIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<OldVault>> + '_> {
        let v: Vec<&dyn Index<OldVault>> = vec![&self.owner];
        Box::new(v.into_iter())
    }
}

pub fn vault_operator_idx(_pk: &[u8], d: &OldVault) -> Addr {
    d.operator.clone()
}

pub fn handle_migrate_vaults(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _start_after: Option<u64>,
    _limit: Option<u64>,
) -> Result<Response, ContractError> {
    ensure!(
        OWNER.is_admin(deps.as_ref(), &info.sender)?,
        ContractError::Unauthorized {}
    );
    nonpayable(&info).map_err(|_| ContractError::NonPayable {})?;

    Ok(Response::default())
}

pub fn handle_migration(_deps: DepsMut, _env: Env) -> Result<Response, ContractError> {
    Ok(Response::default())
}
