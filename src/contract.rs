#[cfg(not(feature = "library"))]
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};
use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};
use cw2::set_contract_version;

const CONTRACT_NAME: &str = "crates.io:cosmwasm-option";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.expires <= env.block.height {
        return Err(ContractError::CustomError {
            val: format!(
                "Option expired, expires: {:?}, block height: {:?}",
                msg.expires, env.block.height
            ),
        });
    }

    let state = State {
        creator: info.sender.clone(),
        owner: info.sender.clone(),
        collateral: info.funds,
        counter_offer: msg.counter_offer,
        expires: msg.expires,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient } => transfer(deps, env, info, recipient),
        ExecuteMsg::Finalize => finalize(deps, env, info),
        ExecuteMsg::Burn => burn(deps, env, info),
    }
}

pub fn transfer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: Addr,
) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.owner = recipient.clone();
        Ok(state)
    })?;
    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("owner", recipient))
}

pub fn finalize(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    if info.sender != state.owner {
        return Err(ContractError::Unauthorized {});
    }
    if env.block.height >= state.expires {
        return Err(ContractError::CustomError {
            val: format!(
                "Option expired, expires: {:?}, block height: {:?}",
                state.expires, env.block.height
            ),
        });
    }
    if info.funds != state.counter_offer {
        return Err(ContractError::CustomError {
            val: format!(
                "Counter offer mismatch, counter offer: {:?}, funds: {:?}",
                state.counter_offer, info.funds
            ),
        });
    }

    STATE.remove(deps.storage);

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: state.creator.to_string(),
            amount: state.counter_offer,
        })
        .add_message(BankMsg::Send {
            to_address: state.owner.to_string(),
            amount: state.collateral,
        })
        .add_attribute("action", "execute"))
}

pub fn burn(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    if state.expires > env.block.height {
        return Err(ContractError::CustomError {
            val: format!(
                "Option not yet expired, expires: {:?}, block height: {:?}",
                state.expires, env.block.height
            ),
        });
    }
    if !info.funds.is_empty() {
        return Err(ContractError::CustomError {
            val: format!("Funds not empty, funds: {:?}", info.funds),
        });
    }

    STATE.remove(deps.storage);

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: state.creator.to_string(),
            amount: state.collateral,
        })
        .add_attribute("action", "burn"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config => {
            let state = STATE.load(deps.storage)?;
            to_binary(&state)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{attr, coins, from_binary, CosmosMsg};

    #[test]
    fn test_instantiate() {
        let counter_offer = coins(40, "ETH");
        let collateral = coins(1, "BTC");
        let mut deps = mock_dependencies_with_balance(&[]);
        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: 100_000,
        };
        let info = mock_info("creator", &collateral);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config).unwrap();
        let state: State = from_binary(&res).unwrap();
        assert_eq!(state.creator, "creator");
        assert_eq!(state.owner, "creator");
        assert_eq!(state.collateral, collateral);
        assert_eq!(state.counter_offer, counter_offer);
        assert_eq!(state.expires, 100_000);
    }

    #[test]
    fn test_transfer() {
        let mut deps = mock_dependencies_with_balance(&[]);
        let msg = InstantiateMsg {
            counter_offer: coins(40, "ETH"),
            expires: 100_000,
        };
        let info = mock_info("creator", &coins(1, "BTC"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &[]);
        let err = transfer(deps.as_mut(), mock_env(), info, Addr::unchecked("anyone")).unwrap_err();
        match err {
            ContractError::Unauthorized {} => {}
            e => panic!("unexpected error: {}", e),
        }

        let info = mock_info("creator", &[]);
        let res = transfer(deps.as_mut(), mock_env(), info, Addr::unchecked("someone")).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(res.attributes[0], attr("action", "transfer"));
        assert_eq!(res.attributes[1], attr("owner", Addr::unchecked("someone")));

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config).unwrap();
        let state: State = from_binary(&res).unwrap();
        assert_eq!(state.creator, "creator");
        assert_eq!(state.owner, "someone");
    }

    #[test]
    fn test_execute() {
        let counter_offer = coins(40, "ETH");
        let collateral = coins(1, "BTC");
        let mut deps = mock_dependencies_with_balance(&[]);
        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: 100_000,
        };
        let info = mock_info("creator", &collateral);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        transfer(deps.as_mut(), mock_env(), info, Addr::unchecked("someone")).unwrap();

        let info = mock_info("creator", &counter_offer);
        let err = finalize(deps.as_mut(), mock_env(), info).unwrap_err();
        match err {
            ContractError::Unauthorized {} => {}
            e => panic!("unexpected error: {}", e),
        }

        let info = mock_info("someone", &counter_offer);
        let mut env = mock_env();
        env.block.height = 200_000;
        let err = finalize(deps.as_mut(), env, info).unwrap_err();
        match err {
            ContractError::CustomError { val } => assert!(val.contains("Option expired")),
            e => panic!("unexpected error: {}", e),
        }

        let info = mock_info("someone", &coins(39, "ETH"));
        let err = finalize(deps.as_mut(), mock_env(), info).unwrap_err();
        match err {
            ContractError::CustomError { val } => assert!(val.contains("Counter offer mismatch")),
            e => panic!("unexpected error: {}", e),
        }

        let info = mock_info("someone", &counter_offer);
        let res = finalize(deps.as_mut(), mock_env(), info).unwrap();
        assert_eq!(res.messages.len(), 2);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: counter_offer,
            })
        );
        assert_eq!(
            res.messages[1].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "someone".to_string(),
                amount: collateral,
            })
        );
        assert_eq!(res.attributes.len(), 1);
        assert_eq!(res.attributes[0], attr("action", "execute"));

        query(deps.as_ref(), mock_env(), QueryMsg::Config).unwrap_err();
    }

    #[test]
    fn test_burn() {
        let counter_offer = coins(40, "ETH");
        let collateral = coins(1, "BTC");
        let mut deps = mock_dependencies_with_balance(&[]);
        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: 100_000,
        };
        let info = mock_info("creator", &collateral);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let err = burn(deps.as_mut(), mock_env(), info).unwrap_err();
        match err {
            ContractError::CustomError { val } => assert!(val.contains("Option not yet expired")),
            e => panic!("unexpected error: {}", e),
        }

        let info = mock_info("creator", &counter_offer);
        let mut env = mock_env();
        env.block.height = 200_000;
        let err = burn(deps.as_mut(), env.clone(), info).unwrap_err();
        match err {
            ContractError::CustomError { val } => assert!(val.contains("Funds not empty")),
            e => panic!("unexpected error: {}", e),
        }

        let info = mock_info("creator", &[]);
        let res = burn(deps.as_mut(), env, info).unwrap();
        assert_eq!(res.messages.len(), 1);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".to_string(),
                amount: collateral,
            })
        );
        assert_eq!(res.attributes.len(), 1);
        assert_eq!(res.attributes[0], attr("action", "burn"));
    }
}
