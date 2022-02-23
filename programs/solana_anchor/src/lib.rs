pub mod utils;
use borsh::{BorshDeserialize,BorshSerialize};
use {
    crate::utils::*,
    anchor_lang::{
        prelude::*,
        solana_program::{
            program_pack::Pack,
            borsh::try_from_slice_unchecked,
            clock::UnixTimestamp,
            program_error::ProgramError,
        },
        Key,
    },
    spl_token::state,
};
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const LENDING_MARKET_SIZE : usize = 32+32;
pub const RESERVE_SIZE : usize = 1+32+32+32+32+32+32+8+8+8+8+16+1+16+1+20;
pub const OBLIGATION_SIZE : usize = 32+32+8+8+1;

#[program]
pub mod solana_anchor {
    use super::*;

    pub fn init_lending_market(
        ctx : Context<InitLendingMarket>,
        ) -> ProgramResult {
        msg!("Processing initialize_lending_market");
        let lending_market = &mut ctx.accounts.lending_market;
        lending_market.owner = ctx.accounts.authority.key();
        lending_market.oracle_program_id = *ctx.accounts.oracle_program_id.key;
        Ok(())
    }

    pub fn set_lending_market_owner(
        ctx : Context<SetLendingMarketOwner>,
        ) -> ProgramResult {
        let lending_market = &mut ctx.accounts.lending_market;
        lending_market.owner=*ctx.accounts.new_owner.key;
        Ok(())
    }

    pub fn init_reserve(
        ctx : Context<InitReserve>,
        _bump : u8,
        _max_borrow_rate_numerator : u64,
        _max_borrow_rate_denominator : u64,
        ) -> ProgramResult {
        msg!("Processing init_reserve");
        
        let lending_market = &mut ctx.accounts.lending_market;
        let liquidity_account : state::Account = state::Account::unpack_from_slice(&ctx.accounts.liquidity_account.data.borrow())?;
        let collateral_account : state::Account = state::Account::unpack_from_slice(&ctx.accounts.collateral_account.data.borrow())?;

        if liquidity_account.mint != *ctx.accounts.liquidity_mint.key {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }

        if collateral_account.mint != *ctx.accounts.collateral_mint.key {
            return Err(LendingError::NotMatchCollateralMint.into());
        }

        if lending_market.oracle_program_id != *ctx.accounts.oracle_price.owner {
            return Err(LendingError::InvalidOracleConfig.into());
        }

        if collateral_account.owner != ctx.accounts.reserve.key() {
            return Err(LendingError::NotMatchCollateralAccount.into())
        }

        if liquidity_account.owner != ctx.accounts.reserve.key() {
            return Err(LendingError::NotMatchLiquidityAccount.into())
        }

        let reserve = &mut ctx.accounts.reserve;
        reserve.lending_market = ctx.accounts.lending_market.key();
        reserve.liquidity_mint = *ctx.accounts.liquidity_mint.key;
        reserve.liquidity_account = *ctx.accounts.liquidity_account.key;
        reserve.liquidity_oracle = *ctx.accounts.oracle_price.key;
        reserve.collateral_mint = *ctx.accounts.collateral_mint.key;
        reserve.collateral_account = *ctx.accounts.collateral_account.key;
        reserve.max_borrow_rate_numerator = _max_borrow_rate_numerator;
        reserve.max_borrow_rate_denominator = _max_borrow_rate_denominator;
        reserve.total_liquidity = 0;
        reserve.total_collateral = 0;
        reserve.is_live = false;
        reserve.bump = _bump;
        Ok(())
    }

    pub fn reserve_live_control(
        ctx : Context<ReserveLiveControl>,
        is_live : bool
        ) -> ProgramResult {
        let reserve = &mut ctx.accounts.reserve;
        if reserve.lending_market != ctx.accounts.lending_market.key() {
            return Err(LendingError::NotMatchLendingMarket.into());
        }
        reserve.is_live = is_live;
        Ok(())
    }

    pub fn init_obligation(
        ctx : Context<InitObligation>,
        _bump : u8,
        ) -> ProgramResult {
        let obligation = &mut ctx.accounts.obligation;
        obligation.owner = *ctx.accounts.owner.key;
        obligation.reserve = *ctx.accounts.reserve.key;
        obligation.input_amount = 0;
        obligation.output_amount = 0;
        obligation.bump = _bump;
        Ok(())
    }

    pub fn deposit_collateral(
        ctx : Context<DepositCollateral>,
        collateral_amount : u64,
        ) -> ProgramResult {
        let obligation = &mut ctx.accounts.obligation;        
        if obligation.reserve != ctx.accounts.reserve.key() {
            return Err(LendingError::NotMatchReserveAddress.into());
        }
        let reserve = &mut ctx.accounts.reserve;
        if reserve.collateral_account != *ctx.accounts.dest_collateral.key {
            return Err(LendingError::NotMatchCollateralAccount.into());
        }
        let source_collateral : state::Account = state::Account::unpack_from_slice(&ctx.accounts.source_collateral.data.borrow())?;
        let dest_collateral : state::Account = state::Account::unpack_from_slice(&ctx.accounts.dest_collateral.data.borrow())?;
        if source_collateral.mint != reserve.collateral_mint {
            return Err(LendingError::NotMatchCollateralMint.into());
        }
        if dest_collateral.mint != reserve.collateral_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        ////////////////////////////////////////////////////
        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.source_collateral.clone(),
                destination : ctx.accounts.dest_collateral.clone(),
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
                amount : collateral_amount,
            }
        )?;
        obligation.input_amount= obligation.input_amount + collateral_amount;
        reserve.total_collateral = reserve.total_collateral + collateral_amount;
        Ok(())
    }

    pub fn withdraw_collateral(
        ctx : Context<WithdrawCollateral>,
        collateral_amount : u64,
        ) -> ProgramResult {
        let obligation = &mut ctx.accounts.obligation;
        let reserve_account_info = ctx.accounts.reserve.to_account_info().clone();        
        if obligation.reserve != ctx.accounts.reserve.key() {
            return Err(LendingError::NotMatchReserveAddress.into());
        }
        let reserve = &mut ctx.accounts.reserve;
        if reserve.lending_market != *ctx.accounts.lending_market.key {
            return Err(LendingError::NotMatchLendingMarket.into());
        }

        if reserve.collateral_account != *ctx.accounts.source_collateral.key {
            return Err(LendingError::NotMatchCollateralAccount.into());
        }
        if reserve.liquidity_oracle != *ctx.accounts.oracle_price.key {
            return Err(LendingError::InvalidOracleConfig.into());
        }
        let source_collateral : state::Account = state::Account::unpack_from_slice(&ctx.accounts.source_collateral.data.borrow())?;
        let dest_collateral : state::Account = state::Account::unpack_from_slice(&ctx.accounts.dest_collateral.data.borrow())?;
        let liquidity_mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.liquidity_mint.data.borrow())?;
        let collateral_mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.collateral_mint.data.borrow())?;
        if reserve.collateral_mint != *ctx.accounts.collateral_mint.key{
            return Err(LendingError::NotMatchCollateralMint.into());
        }
        if reserve.liquidity_mint != *ctx.accounts.liquidity_mint.key {
            return Err(LendingError::NotMatchCollateralMint.into());
        }
        if source_collateral.mint != reserve.collateral_mint {
            return Err(LendingError::NotMatchCollateralMint.into());
        }
        if dest_collateral.mint != reserve.collateral_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        ////////////////////////////////////////////////////

        //Can I borrow?
        if collateral_amount > source_collateral.amount {
            return Err(LendingError::NotEnoughCollateral.into());
        }
        let mut real_amount = collateral_amount;
        if collateral_amount > obligation.input_amount {
            real_amount = obligation.input_amount;
        }

        if (
            obligation.output_amount as u128
                * reserve.liquidity_market_price as u128 
                * reserve.max_borrow_rate_denominator as u128 
                / 10u128.pow((liquidity_mint.decimals + reserve.liquidity_market_price_decimals) as u32)
            )
            >
           (
            (obligation.input_amount - real_amount) as u128
                * reserve.collateral_market_price as u128
                * reserve.max_borrow_rate_numerator as u128 
                / 10u128.pow((collateral_mint.decimals + reserve.collateral_market_price_decimals) as u32)
            ) 
           {
            return Err(LendingError::InvalidBorrowRate.into());
        }

        let lending_seeds = &[
            ctx.accounts.lending_market.key.as_ref(),
            reserve.collateral_mint.as_ref(),
            reserve.liquidity_mint.as_ref(),
            &[reserve.bump]
        ];

        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.source_collateral.clone(),
                destination : ctx.accounts.dest_collateral.clone(),
                authority : reserve_account_info,
                token_program : ctx.accounts.token_program.clone(),
                authority_signer_seeds : lending_seeds,
                amount : real_amount,
            }
        )?;
        obligation.input_amount= obligation.input_amount - real_amount;
        reserve.total_collateral = reserve.total_collateral - real_amount;
        Ok(())
    }

    pub fn borrow_liquidity(
        ctx : Context<BorrowLiquidity>,
        liquidity_amount : u64,
        ) -> ProgramResult {
        let reserve_account_info = ctx.accounts.reserve.to_account_info().clone(); 
        let obligation = &mut ctx.accounts.obligation;
        if obligation.reserve != ctx.accounts.reserve.key() {
            return Err(LendingError::NotMatchReserveAddress.into());
        }
        let reserve = &mut ctx.accounts.reserve;
        if reserve.lending_market != *ctx.accounts.lending_market.key {
            return Err(LendingError::NotMatchLendingMarket.into());
        }
        if reserve.liquidity_account != *ctx.accounts.source_liquidity.key {
            return Err(LendingError::NotMatchLiquidityAccount.into());
        }
        if reserve.liquidity_oracle != *ctx.accounts.oracle_price.key {
            return Err(LendingError::InvalidOracleConfig.into());
        }
        let source_liquidity : state::Account = state::Account::unpack_from_slice(&ctx.accounts.source_liquidity.data.borrow())?;
        let dest_liquidity : state::Account = state::Account::unpack_from_slice(&ctx.accounts.dest_liquidity.data.borrow())?;
        let liquidity_mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.liquidity_mint.data.borrow())?;
        let collateral_mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.collateral_mint.data.borrow())?;
        if reserve.collateral_mint != *ctx.accounts.collateral_mint.key{
            return Err(LendingError::NotMatchCollateralMint.into());
        }
        if reserve.liquidity_mint != *ctx.accounts.liquidity_mint.key {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        if source_liquidity.mint != reserve.liquidity_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        if dest_liquidity.mint != reserve.liquidity_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        ////////////////////////////////////////////////

        //Can I borrow?
        if liquidity_amount > source_liquidity.amount {
            return Err(LendingError::NotEnoughLiquidity.into());
        }

        if (
            (obligation.output_amount + liquidity_amount) as u128
                * reserve.liquidity_market_price as u128 
                * reserve.max_borrow_rate_denominator as u128 
                / 10u128.pow((liquidity_mint.decimals + reserve.liquidity_market_price_decimals) as u32)
            )
            >
           (
            obligation.input_amount as u128
                * reserve.collateral_market_price as u128
                * reserve.max_borrow_rate_numerator as u128 
                / 10u128.pow((collateral_mint.decimals + reserve.collateral_market_price_decimals) as u32)
            ) 
           {
            return Err(LendingError::InvalidBorrowRate.into());
        }

        let lending_seeds = &[
            ctx.accounts.lending_market.key.as_ref(),
            reserve.collateral_mint.as_ref(),
            reserve.liquidity_mint.as_ref(),
            &[reserve.bump]
        ];     

        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.source_liquidity.clone(),
                destination : ctx.accounts.dest_liquidity.clone(),
                authority : reserve_account_info,
                authority_signer_seeds : lending_seeds,
                token_program : ctx.accounts.token_program.clone(),
                amount : liquidity_amount,
            }
        )?;
        obligation.output_amount = obligation.output_amount + liquidity_amount;
        reserve.total_liquidity = reserve.total_liquidity + liquidity_amount;
        Ok(())
    }

    pub fn repay_liquidity(
        ctx : Context<RepayLiquidity>,
        liquidity_amount : u64,
        ) -> ProgramResult{
        let obligation = &mut ctx.accounts.obligation;
        if obligation.reserve != ctx.accounts.reserve.key() {
            return Err(LendingError::NotMatchReserveAddress.into());
        }
        let reserve = &mut ctx.accounts.reserve;
        if reserve.liquidity_account != *ctx.accounts.dest_liquidity.key {
            return Err(LendingError::NotMatchLiquidityAccount.into());
        }
        let source_liquidity : state::Account = state::Account::unpack_from_slice(&ctx.accounts.source_liquidity.data.borrow())?;
        let dest_liquidity : state::Account = state::Account::unpack_from_slice(&ctx.accounts.dest_liquidity.data.borrow())?;
        if source_liquidity.mint != reserve.liquidity_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        if dest_liquidity.mint != reserve.liquidity_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        ///////////////////////////////////////////////        
        let mut real_amount : u64 = liquidity_amount;
        if real_amount > obligation.output_amount {
            real_amount = obligation.output_amount;
        }
        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.source_liquidity.clone(),
                destination : ctx.accounts.dest_liquidity.clone(),
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
                amount : real_amount,
            }
        )?;
        obligation.output_amount = obligation.output_amount - real_amount;
        reserve.total_liquidity = reserve.total_liquidity - real_amount;
        Ok(())
    }

    pub fn redeem_reserve_collateral(
        ctx : Context<RedeemReserveCollateral>,
        amount : u64,
        ) -> ProgramResult {
        let reserve_account_info = ctx.accounts.reserve.to_account_info().clone();
        let reserve = &mut ctx.accounts.reserve;
        if reserve.lending_market != ctx.accounts.lending_market.key() {
            return Err(LendingError::NotMatchLendingMarket.into());
        }
        if reserve.collateral_account != *ctx.accounts.source_collateral.key {
            return Err(LendingError::NotMatchCollateralAccount.into());
        }
        let source_collateral : state::Account = state::Account::unpack_from_slice(&ctx.accounts.source_collateral.data.borrow())?;
        let dest_collateral : state::Account = state::Account::unpack_from_slice(&ctx.accounts.dest_collateral.data.borrow())?;
        if source_collateral.mint != reserve.collateral_mint {
            return Err(LendingError::NotMatchCollateralMint.into());
        }
        if dest_collateral.mint != reserve.collateral_mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }
        if source_collateral.amount < amount {
            return Err(LendingError::NotEnoughCollateral.into());
        }

        let lending_seeds = &[
            ctx.accounts.lending_market.to_account_info().key.as_ref(),
            reserve.collateral_mint.as_ref(),
            reserve.liquidity_mint.as_ref(),
            &[reserve.bump]
        ];
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.source_collateral.clone(),
                destination : ctx.accounts.dest_collateral.clone(),
                authority : reserve_account_info,
                authority_signer_seeds : lending_seeds,
                token_program : ctx.accounts.token_program.clone(),
                amount : amount,
            }
        )?;
        Ok(())
    }

    pub fn deposit_reserve_liquidity(
        ctx : Context<DepositReserveLiquidity>,
        _amount : u64,
        ) -> ProgramResult {
        let reserve = &mut ctx.accounts.reserve;
        if reserve.liquidity_account != *ctx.accounts.dest_liquidity.key {
            return Err(LendingError::NotMatchLiquidityAccount.into());
        }
        let source_liquidity : state::Account = state::Account::unpack_from_slice(&ctx.accounts.source_liquidity.data.borrow())?;
        // let dest_liquidity   : state::Account = state::Account::unpack_from_slice(&ctx.accounts.dest_liquidity.data.borrow())?;
        if reserve.liquidity_mint != source_liquidity.mint {
            return Err(LendingError::NotMatchLiquidityMint.into());
        }

        spl_token_transfer_without_seed(
            TokenTransferParamsWithoutSeed{
                source : ctx.accounts.source_liquidity.clone(),
                destination : ctx.accounts.dest_liquidity.clone(),
                authority : ctx.accounts.owner.clone(),
                token_program : ctx.accounts.token_program.clone(),
                amount : _amount,
            }
        )?;

        Ok(())
    }

    pub fn set_borrow_rate(
        ctx : Context<SetBorrowRate>,
        _borrow_rate_numerator : u64,
        _borrow_rate_denominator : u64
        ) -> ProgramResult {
        let reserve = &mut ctx.accounts.reserve;
        if reserve.lending_market != ctx.accounts.lending_market.key() {
            return Err(LendingError::NotMatchLendingMarket.into());
        }
        reserve.max_borrow_rate_numerator=_borrow_rate_numerator;
        reserve.max_borrow_rate_denominator=_borrow_rate_denominator;
        Ok(())
    }

    pub fn set_market_price(
        ctx : Context<SetMarketPrice>,
        _collateral_market_price : u128,
        _collateral_market_price_decimals : u8,
        ) -> ProgramResult {
        let reserve = &mut ctx.accounts.reserve;
        if reserve.lending_market != ctx.accounts.lending_market.key() {
            return Err(LendingError::NotMatchLendingMarket.into());
        }
        if reserve.liquidity_oracle != *ctx.accounts.oracle_price.key {
            return Err(LendingError::InvalidOracleConfig.into());
        }

        let aggregator : Aggregator = try_from_slice_unchecked(&ctx.accounts.oracle_price.data.borrow()[..4096])?;
        let mut price : u128 = 0;
        if let Some(answer) = aggregator.answer {
            price = answer;
        }else {
            return Err(LendingError::InvalidOracleConfig.into());
        }

        reserve.liquidity_market_price = price;
        reserve.liquidity_market_price_decimals = aggregator.config.decimals;

        reserve.collateral_market_price = _collateral_market_price;
        reserve.collateral_market_price_decimals = _collateral_market_price_decimals;
        
        // reserve.is_live = 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetMarketPrice<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(has_one=owner)]
    lending_market : ProgramAccount<'info,LendingMarket>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    oracle_price : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SetBorrowRate<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(has_one=owner)]
    lending_market : ProgramAccount<'info,LendingMarket>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,
}

#[derive(Accounts)]
pub struct DepositReserveLiquidity<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    source_liquidity : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    dest_liquidity : AccountInfo<'info>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RedeemReserveCollateral<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    source_collateral : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    dest_collateral : AccountInfo<'info>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(has_one=owner)]
    lending_market : ProgramAccount<'info,LendingMarket>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RepayLiquidity<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    source_liquidity : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    dest_liquidity : AccountInfo<'info>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(mut,seeds=[reserve.key().as_ref(),(*owner.key).as_ref()], bump=obligation.bump,has_one=owner)]
    obligation : ProgramAccount<'info,Obligation>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,   
}

#[derive(Accounts)]
pub struct BorrowLiquidity<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    collateral_mint : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    liquidity_mint : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    source_liquidity : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    dest_liquidity : AccountInfo<'info>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(mut,seeds=[reserve.key().as_ref(),(*owner.key).as_ref()], bump=obligation.bump,)]
    obligation : ProgramAccount<'info,Obligation>,

    lending_market : AccountInfo<'info>,

    oracle_price : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct WithdrawCollateral<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    collateral_mint : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    liquidity_mint : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    source_collateral : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    dest_collateral : AccountInfo<'info>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(mut,seeds=[reserve.key().as_ref(),(*owner.key).as_ref()], bump=obligation.bump,has_one=owner)]
    obligation : ProgramAccount<'info,Obligation>,

    lending_market : AccountInfo<'info>,

    oracle_price : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DepositCollateral<'info> {
    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    source_collateral : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    dest_collateral : AccountInfo<'info>,

    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(mut,seeds=[reserve.key().as_ref(),(*owner.key).as_ref()], bump=obligation.bump, has_one=owner)]
    obligation : ProgramAccount<'info,Obligation>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct InitObligation<'info> {
    #[account(init, seeds=[reserve.key().as_ref(),(*owner.key).as_ref()], bump=_bump, payer=owner, space=8+OBLIGATION_SIZE)]
    obligation : ProgramAccount<'info,Obligation>,

    #[account(mut,signer)]
    owner : AccountInfo<'info>,

    reserve : AccountInfo<'info>,

    system_program : Program<'info,System>,
}

// seeds=[lending_market.key().as_ref(),(*liquidity_mint.key).as_ref(),(*collateral_mint.key).as_ref()], bump=bump ,
// #[instruction(bump : u8,_max_borrow_rate : u8)]
#[derive(Accounts)]
#[instruction(_bump : u8, _max_borrow_rate : u64)]
pub struct InitReserve<'info> {
    #[account(init, 
        seeds=[lending_market.key().as_ref(),(*collateral_mint.key).as_ref(),(*liquidity_mint.key).as_ref()],
        bump=_bump,
        payer=owner, space=8+RESERVE_SIZE)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(mut)]
    owner : Signer<'info>,

    #[account(mut,has_one=owner)]
    lending_market : ProgramAccount<'info,LendingMarket>,

    #[account(owner=spl_token::id())]
    liquidity_mint : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    liquidity_account : AccountInfo<'info>,

    oracle_price : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    collateral_mint : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    collateral_account : AccountInfo<'info>,

    system_program : Program<'info,System>,
}

#[derive(Accounts)]
pub struct SetLendingMarketOwner<'info> {
    #[account(mut, has_one=owner)]
    lending_market : ProgramAccount<'info,LendingMarket>,

    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account(mut)]
    new_owner : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ReserveLiveControl<'info> {
    #[account(mut)]
    reserve : ProgramAccount<'info,Reserve>,

    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account(mut,has_one=owner)]
    lending_market : ProgramAccount<'info,LendingMarket>,
}

#[derive(Accounts)]
pub struct InitLendingMarket<'info> {
    #[account(init, payer=authority, space=8+LENDING_MARKET_SIZE)]
    lending_market : ProgramAccount<'info,LendingMarket>,

    #[account(mut)]
    authority : Signer<'info>,

    oracle_program_id : AccountInfo<'info>,

    system_program : Program<'info,System>
}

#[account]
pub struct LendingMarket{
    pub owner : Pubkey,
    pub oracle_program_id : Pubkey,
}

#[account]
pub struct Reserve{
    pub is_live : bool,
    pub lending_market : Pubkey,
    pub liquidity_mint : Pubkey,
    pub liquidity_account : Pubkey,
    pub liquidity_oracle : Pubkey,
    pub collateral_mint : Pubkey,
    pub collateral_account : Pubkey,
    pub total_liquidity : u64,
    pub total_collateral : u64,
    pub max_borrow_rate_numerator : u64,
    pub max_borrow_rate_denominator : u64,
    pub liquidity_market_price : u128,
    pub liquidity_market_price_decimals : u8,
    pub collateral_market_price : u128,
    pub collateral_market_price_decimals : u8,
    pub bump : u8,
}

#[account]
pub struct Obligation{
    pub reserve : Pubkey,
    pub owner : Pubkey,
    pub input_amount : u64,
    pub output_amount : u64,
    pub bump : u8,
}

#[error]
pub enum LendingError {
    #[msg("Pyth product account provided is not owned by the lending market oracle program")]
    InvalidOracleConfig,

    #[msg("Math operation overflow")]
    MathOverflow,

    #[msg("Not match liquidity account")]
    NotMatchLiquidityAccount,

    #[msg("Not match liquidity mint")]
    NotMatchLiquidityMint,

    #[msg("Not match owner address")]
    NotMatchOwnerAddress,

    #[msg("Not match collateral mint")]
    NotMatchCollateralMint,

    #[msg("Not match collateral account")]
    NotMatchCollateralAccount,

    #[msg("Not match reserve address")]
    NotMatchReserveAddress,

    #[msg("Token transfer failed")]
    TokenTransferFailed,

    #[msg("Token set authority failed")]
    TokenSetAuthorityFailed,

    #[msg("Not enough liquidity")]
    NotEnoughLiquidity,

    #[msg("Invalid borrow rate")]
    InvalidBorrowRate,

    #[msg("Not enough collateral")]
    NotEnoughCollateral,

    #[msg("Not match lending market")]
    NotMatchLendingMarket,

    #[msg("Derived key invalid")]
    DerivedKeyInvalid,
}

#[derive(Clone, Copy, Eq, PartialEq, BorshSerialize, BorshDeserialize, Default, Debug)]
#[repr(C)]
pub struct Submission(pub UnixTimestamp,pub u128);

#[derive(Clone, Eq, PartialEq, BorshSerialize, BorshDeserialize, Default, Debug)]
#[repr(C)]
pub struct Aggregator {
    pub is_initialize : bool,
    pub version : u32,
    pub config : Config,
    pub updated_at : UnixTimestamp,
    pub owner : Pubkey,
    pub submissions : [Submission;8],
    pub answer : Option<u128>,
}

#[derive(Clone,Eq, PartialEq, BorshSerialize, BorshDeserialize, Default, Debug)]
#[repr(C)]
pub struct Config {
    pub oracles : Vec<Pubkey>,
    pub min_answer_threshold : u8,
    pub staleness_threshold : u8,
    pub decimals : u8,
}
