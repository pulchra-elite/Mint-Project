use std::env;

use cosmwasm_std::{
    to_binary, Api, Binary, Env, Extern, HandleResponse, InitResponse, Querier,
    StdError, StdResult, Storage, Uint128, HumanAddr,Decimal, from_binary
};
use secret_toolkit::snip721::{Metadata, Extension,Trait};


use crate::msg::{ HandleMsg, InitMsg, QueryMsg,Wallet, MetadataMsg};
use crate::state::{config, config_read, State, store_members, read_members, store_user_info,read_user_info};
use secret_toolkit::{snip20,snip721};
pub const RESPONSE_BLOCK_SIZE: usize = 256;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let state = State {
        count: Uint128(0),
        total_supply:msg.total_supply,
        admin: msg.admin,
        maximum_count : msg.maximum_count,
        public_price : msg.public_price,
        private_price :msg.private_price,
        reward_wallet : msg.reward_wallet,
        presale_start : msg.presale_start,
        presale_period : msg.presale_period,
        can_mint : true,
        nft_address:HumanAddr::from("nft_address"),
        denom : msg.denom,
        token_address:msg.token_address,
        token_contract_hash:msg.token_contract_hash,
        check_minted : msg.check_minted
    };

    config(&mut deps.storage).save(&state)?;
    store_members(&mut deps.storage).save(&msg.white_members)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive { sender,from,amount,msg} => mint_nft(deps,env,sender,from,amount,msg),
        HandleMsg::SetMaximumNft { amount } => set_maximum_nft(deps, env,amount),
        HandleMsg::SetTotalSupply { amount } => set_total_supply(deps, env,amount),
        HandleMsg::ChangeAdmin { address } => set_admin(deps,env,address),
        HandleMsg::SetRewardWallet { wallet } => set_reward_wallet(deps,env,wallet),
        HandleMsg::SetPrice { public_price, private_price} =>set_price(deps,env,public_price,private_price),
        HandleMsg::SetMintFlag { flag } => set_flag(deps,env,flag),
        HandleMsg::SetMintTime { presale_start, presale_period }=> set_mint_time(deps,env,presale_start,presale_period),
        HandleMsg::SetWhiteUsers { members } => set_white_members(deps,env,members),
        HandleMsg::AddWhiteUser { member } => add_white_user(deps,env,member),
        HandleMsg::SetNftAddress { nft_address } => set_nft_address(deps,env,nft_address),
        HandleMsg::SetTokenAddres{token_address,token_contract_hash} => set_token_address(deps,env,token_address,token_contract_hash)
    }
}

pub fn mint_nft<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender:HumanAddr,
    from:HumanAddr,
    amount:Uint128,
    msg:Binary
) -> StdResult<HandleResponse> {
    let metadata :MetadataMsg = from_binary(&msg)?;
    let state = config_read(&deps.storage).load()?;
    let crr_time =  env.block.time;
    if crr_time < state.presale_start{
        return Err(StdError::generic_err(
            "PresaleNotStarted"
        ))
    }

     if state.token_address != env.message.sender{
        return Err(StdError::generic_err(
            "Wrong contract address"
        ))
    }

    if state.total_supply <= state.count{
        return Err(StdError::generic_err(
            "Can not mint any more"
        ))
    }

     if state.can_mint == false{
        return Err(StdError::generic_err(
            "Can not mint for some time"
        ))
    }



    config(&mut deps.storage).update(|mut state| {
            state.count = state.count+Uint128(1);
            state.check_minted[(metadata.number.unwrap()-1) as usize] = false;  
            Ok(state)
        })?;


    if crr_time<(state.presale_start + state.presale_period) {
        let members = read_members(&deps.storage).load()?;
        let mut flag = false;
        for member in members {
            if member == sender {
                flag = true;
            }
        }
        if !flag{
            return Err(StdError::generic_err(
                "You are not whitelisted")
            )
        }
        if amount != state.private_price{
            return Err(StdError::generic_err(
                "Not exact money"
            ))
        }
        let user_info = read_user_info(&deps.storage,&sender.as_str());
        
        let token_id = metadata.clone().tokenId.unwrap(); 
        if user_info == None{
            store_user_info(& mut deps.storage, &sender.as_str(), vec![token_id.clone()])?;
        }
        else {
            let mut new_user_info = user_info.unwrap();
            if Uint128(new_user_info.len() as u128) >= state.maximum_count{
                return Err(StdError::generic_err(
                    "You can not mint any more"
                ))
            }
            new_user_info.push(token_id.clone());
            store_user_info(& mut deps.storage, &sender.as_str(), new_user_info)?;
        }

    

        let mut res = vec![
           snip721::mint_nft_msg(Some(token_id),
            Some(sender), 
            Some(Metadata{
                token_uri:None,
                extension:Some(Extension{
                    image:metadata.image,
                    image_data:None,
                    external_url:None,
                    description:metadata.description,
                    name:metadata.name,
                    attributes:metadata.attributes,
                    background_color:None,
                    animation_url:None,
                    youtube_url:None,
                    media:None,
                    protected_attributes:None
                })
            }),
            None, None, None,
            RESPONSE_BLOCK_SIZE, 
            metadata.code_hash.unwrap(), 
            state.nft_address)?
        ];
        
        for reward_member in state.reward_wallet{
            res.push(
                snip20::transfer_msg(reward_member.address, 
                    amount*reward_member.portion, 
                    None,
                    None,
                    RESPONSE_BLOCK_SIZE, 
                    state.token_contract_hash.clone(), 
                    state.token_address.clone())?
            )
        }
          Ok(HandleResponse {
             messages:res,
              log: vec![],
             data: None,
         })
    }
    else{
        if amount != state.public_price{
            return Err(StdError::generic_err(
                "Not exact money"
            ))
        }
        let user_info = read_user_info(&deps.storage,&sender.as_str());
        let token_id = metadata.tokenId.unwrap(); 
        if user_info == None{
            store_user_info(& mut deps.storage, &sender.as_str(), vec![token_id.clone()])?;
        }
        else {
            let mut new_user_info = user_info.unwrap();
            
            new_user_info.push(token_id.clone());
            store_user_info(& mut deps.storage, &sender.as_str(), new_user_info)?;
        }
       
        let mut  res = vec![
           snip721::mint_nft_msg(Some(token_id),
            Some(sender), 
            Some(Metadata{
                token_uri:None,
                extension:Some(Extension{
                    image:metadata.image,
                    image_data:None,
                    external_url:None,
                    description:metadata.description,
                    name:metadata.name,
                    attributes:metadata.attributes,
                    background_color:None,
                    animation_url:None,
                    youtube_url:None,
                    media:None,
                    protected_attributes:None
                })
            }),
            None, None, None,
            RESPONSE_BLOCK_SIZE, 
             metadata.code_hash.unwrap(), 
            state.nft_address)?
        ];
        
        
         for reward_member in state.reward_wallet{
            res.push(
                snip20::transfer_msg(reward_member.address, 
                    amount*reward_member.portion, 
                    None,
                    None,
                    RESPONSE_BLOCK_SIZE, 
                    state.token_contract_hash.clone(), 
                    state.token_address.clone())?
            )
        }
          Ok(HandleResponse {
             messages:res,
              log: vec![],
             data: None,
         })
    }

    
}


pub fn set_maximum_nft<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    amount:Uint128
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.maximum_count = amount;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}

pub fn set_total_supply<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    amount:Uint128
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.total_supply = amount;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}

pub fn set_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    address:HumanAddr
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.admin = address;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}



pub fn set_nft_address<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    address:HumanAddr
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.nft_address = address;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}

pub fn set_token_address<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    address:HumanAddr,
    token_contract_hash:String
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.token_address = address;
        state.token_contract_hash = token_contract_hash;
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}


pub fn set_reward_wallet<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    wallet:Vec<Wallet>
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    let mut portion = Decimal::zero();
    for personal_wallet in wallet.clone(){
        portion = personal_wallet.portion + portion;
    }

    if portion != Decimal::one(){
        return Err(StdError::generic_err("The sum must be equal to 1"))
    };

    config(&mut deps.storage).update(|mut state| {
        state.reward_wallet = wallet;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}

pub fn set_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    public_price:Uint128,
    private_price:Uint128
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.public_price = public_price;
        state.private_price = private_price;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}


pub fn set_flag<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    flag:bool
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.can_mint = flag;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}

pub fn set_mint_time<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    presale_start:u64,
    presale_period:u64
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    config(&mut deps.storage).update(|mut state| {
        state.presale_start = presale_start;
        state.presale_period = presale_period;
     
        Ok(state)
    })?;

   
    Ok(HandleResponse::default())
}

pub fn set_white_members<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    new_members:Vec<HumanAddr>
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    store_members(&mut deps.storage).save(&new_members)?;

    Ok(HandleResponse::default())
}


pub fn add_white_user<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    member:HumanAddr
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;
    if _env.message.sender != state.admin{
        return Err(StdError::generic_err(
            "Unauthorized"
        ))
    }
    let crr_members = read_members(&deps.storage).load()?;
      for crr_member in crr_members{
        if crr_member == member{
         return Err(StdError::generic_err("repeated user"));         
        }
    }
    store_members(&mut deps.storage).update(|mut members| {
        members.push(member);
     
        Ok(members)
    })?;

    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetStateInfo {} => to_binary(&query_count(deps)?),
        QueryMsg::GetWhiteUsers {} => to_binary(&query_white_users(deps)?),
        QueryMsg::GetUserInfo { address } => to_binary(&query_user_info(deps,address)?),

    }
}

fn query_count<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> StdResult<State> {
    let state = config_read(&deps.storage).load()?;
    Ok(state)
}



fn query_white_users<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> StdResult<Vec<HumanAddr>> {
    let members = read_members(&deps.storage).load()?;
    Ok(members)
}

fn query_user_info<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>,address:HumanAddr) -> StdResult<Vec<String>> {
    let user_info  = read_user_info(&deps.storage,&address.as_str());
    if user_info == None{
        Ok(vec![])
    }
    else{
    Ok(user_info.unwrap())
    }
}

#[cfg(test)]
mod tests {


    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coins, from_binary, StdError};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("creator", &coins(1000, "earth"));

        let msg = InitMsg {
             white_members: vec![HumanAddr::from("white1"),HumanAddr::from("white2")],
             admin : HumanAddr::from("admin"),
             total_supply : Uint128(5),
             maximum_count :Uint128(1),
             public_price : Uint128(500000),
             private_price:Uint128(600000) ,
             reward_wallet : vec![Wallet{
                 address:HumanAddr::from("reward1"),
                 portion:Decimal::from_ratio(7 as u128,100 as u128)
             },
             Wallet{
                 address:HumanAddr::from("reward2"),
                 portion:Decimal::from_ratio(3 as u128,100 as u128)
             }
             ],
             presale_start : env.block.time,
             presale_period:100,
             denom:"uscrt".to_string(),
             token_address:HumanAddr::from("token_address"),
             token_contract_hash :"token_hash".to_string(),
             check_minted : vec![true,true,true,true,true]
            };
        
        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());
        // assert_eq!(from_binary(&Binary::from("W29iamVjdCBPYmplY3Rd"))?,[])
        // let crr_time = query_get_time(&deps,env.clone()).unwrap();
        // assert_eq!(env.block.time,crr_time)
       
    }

    #[test]
    fn test_state() {
         let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("creator", &vec![]);

        let msg = InitMsg {
             white_members: vec![HumanAddr::from("white1"),HumanAddr::from("white2")],
             admin : HumanAddr::from("admin"),
             total_supply : Uint128(5),
             maximum_count :Uint128(1),
             public_price : Uint128(500000),
             private_price:Uint128(600000) ,
             reward_wallet : vec![Wallet{
                 address:HumanAddr::from("reward1"),
                 portion:Decimal::from_ratio(7 as u128,100 as u128)
             },
             Wallet{
                 address:HumanAddr::from("reward2"),
                 portion:Decimal::from_ratio(3 as u128,100 as u128)
             }
             ],
             presale_start : env.block.time,
             presale_period:100,
             denom:"uscrt".to_string(),
              token_address:HumanAddr::from("token_address"),
              token_contract_hash :"token_hash".to_string(),
                check_minted : vec![true,true,true,true,true]
            };
        
        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());
        
        // let env = mock_env("anyone", &coins(2, "token"));
        // let msg = HandleMsg::Increment {};
        // let _res = handle(&mut deps, env, msg).unwrap();

        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetRewardWallet { wallet: vec![Wallet{
            address:HumanAddr::from("reward1"),
            portion:Decimal::from_ratio(100 as u128,100 as u128)
        }] };
        let _res = handle(&mut deps, env, msg).unwrap();
        let state = query_count(&deps).unwrap();

        assert_eq!(state.reward_wallet,vec![Wallet{
            address:HumanAddr::from("reward1"),
            portion:Decimal::one()
        }]);

        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetMaximumNft { amount: Uint128(2) };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
        assert_eq!(state.maximum_count,Uint128(2));

        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetTotalSupply { amount: Uint128(100) };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
        assert_eq!(state.total_supply,Uint128(100));

        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetMintFlag { flag :false };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
        assert_eq!(state.can_mint,false);

        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetPrice { public_price: Uint128(5), private_price: Uint128(10) };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
        assert_eq!(state.public_price,Uint128(5));
        assert_eq!(state.private_price,Uint128(10));

        
        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetMintTime { presale_start: env.block.time+10, presale_period: 200 };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
    
        assert_eq!(state.presale_period,200);

        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::ChangeAdmin { address: HumanAddr::from("admin1") };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
        assert_eq!(state.admin,HumanAddr::from("admin1"));


        let env = mock_env("admin1", &vec![]);
        let msg = HandleMsg::SetWhiteUsers { members:vec![HumanAddr::from("white1")] };
        let _res = handle(&mut deps, env, msg).unwrap();

        let members = query_white_users(&deps).unwrap();
        assert_eq!(members,vec![HumanAddr::from("white1")]);


        let env = mock_env("admin1", &vec![]);
        let msg = HandleMsg::AddWhiteUser { member: HumanAddr::from("white2") };
        let _res = handle(&mut deps, env, msg).unwrap();

        let members = query_white_users(&deps).unwrap();
        assert_eq!(members,vec![HumanAddr::from("white1"),HumanAddr::from("white2")]);

    }

     #[test]
    fn mint() {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("creator", &coins(1000, "earth"));

        let msg = InitMsg {
             white_members: vec![HumanAddr::from("white1"),HumanAddr::from("white2")],
             admin : HumanAddr::from("admin"),
             total_supply : Uint128(3),
             maximum_count :Uint128(1),
             public_price : Uint128(600000),
             private_price:Uint128(400000) ,
             reward_wallet : vec![Wallet{
                 address:HumanAddr::from("reward1"),
                 portion:Decimal::from_ratio(70 as u128,100 as u128)
             },
             Wallet{
                 address:HumanAddr::from("reward2"),
                 portion:Decimal::from_ratio(30 as u128,100 as u128)
             }
             ],
             presale_start : env.block.time-110,
             presale_period:100,
             denom:"uscrt".to_string(),
              token_address:HumanAddr::from("token_address"),
              token_contract_hash :"token_hash".to_string(),
                check_minted : vec![true,true,true,true,true]
            };
        
        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());
        
        let env = mock_env("admin", &vec![]);
        let msg = HandleMsg::SetNftAddress { nft_address: HumanAddr::from("nft") };
        let _res = handle(&mut deps, env, msg).unwrap();

        let state = query_count(&deps).unwrap();
        assert_eq!(state.nft_address,HumanAddr::from("nft"));

        let message = to_binary( 
             &MetadataMsg{          
            tokenId:Some("punks1".to_string()),
            description:Some("secret steam".to_string()),
            attributes : Some(vec![Trait{
                trait_type:Some("Clothes".to_string()),
                value:"value".to_string(),
                display_type:None,
                max_value:None
            }]),
            name:Some("name".to_string()),
            image:Some("image".to_string()),
            protected_attributes:None,
            code_hash:Some("CodeHash".to_string()),
            number:Some(3)
        }).unwrap();
        let env = mock_env("token_address", &vec![]);
         let msg = HandleMsg::Receive { sender: HumanAddr::from("white1"), from: HumanAddr::from("xxx"), amount: Uint128(600000), msg: message.clone() };

        
        let _res = handle(&mut deps, env, msg).unwrap();
        let user_info = query_user_info(&deps, HumanAddr::from("minter2")).unwrap();
        let empty : Vec<String> = vec![];
        assert_eq!(user_info,empty);
        
        let user_info = query_user_info(&deps, HumanAddr::from("white1")).unwrap();
        assert_eq!(user_info,vec!["punks1".to_string()]);
       
       let env = mock_env("token_address", &vec![]);
         let msg = HandleMsg::Receive { sender: HumanAddr::from("white1"), from: HumanAddr::from("xxx"), amount: Uint128(600000), msg: message.clone() };
         let _res = handle(&mut deps, env, msg).unwrap();
       
        }

    
}
