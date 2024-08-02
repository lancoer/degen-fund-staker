use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

#[account]
#[derive(Default)]
pub struct StakingAccount {
    pub initializer_key: Pubkey,
    pub lock_end_date: u64,
    pub total_x_token: u64,
    pub freeze_program: bool,
}

#[account]
#[derive(Default)]
pub struct UserStakingAccount {
    pub amount: u64,
    pub x_token_amount: u64,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        address = crate::constants::PLENTY_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub stake_token_mint: Box<Account<'info, Mint>>,

    /// the token account to withdraw from
    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub stake_token_account: Box<Account<'info, TokenAccount>>,

    /// the token vault to stake
    #[account(
        init_if_needed,
        payer = user,
        token::mint = stake_token_mint,
        token::authority = stake_token_vault,
    )]
    pub stake_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        seeds = [ crates::constants::STAKING_SEED.as_ref() ],
        bump,
        space = StakingAccount::Len,
        constraint = !staking.freeze_program,
    )]
    pub staking: Account<'info, StakingAccount>,

    #[account(
        init_if_needed,
        payer = user,
        seeds = [ user.key().as_ref() ],
        bump,
        space = UserStakingAccount::LEN,
    )]
    pub user_staking: Account<'info, UserStakingAccount>,

    /// stake fee account
    #[account(
        mut,
        address = crate::constants::STAKE_FEE_RECEIVER.parse::<Pubkey>().unwrap(),
    )]
    pub stake_fee: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

// #[derive(Accounts)]
// #[instruction(_nonce_vault: u8, _nonce_staking: u8, _nonce_user_staking: u8)]
// pub struct Stake<'info> {
//     #[account(
//         address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
//     )]
//     pub token_mint: Box<Account<'info, Mint>>,

//     #[account(mut)]
//     //the token account to withdraw from
//     pub token_from: Box<Account<'info, TokenAccount>>,

//     //the authority allowed to transfer from token_from
//     pub token_from_authority: Signer<'info>,

//     #[account(
//         mut,
//         seeds = [ token_mint.key().as_ref() ],
//         bump = _nonce_vault,
//     )]
//     pub token_vault: Box<Account<'info, TokenAccount>>,

//     #[account(
//         mut,
//         seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
//         bump = _nonce_staking,
//         constraint = !staking_account.freeze_program,
//     )]
//     pub staking_account: ProgramAccount<'info, StakingAccount>,

//     #[account(
//         init_if_needed,
//         payer = token_from_authority,
//         seeds = [ token_from_authority.key().as_ref() ],
//         bump = _nonce_user_staking,
//     )]
//     pub user_staking_account: ProgramAccount<'info, UserStakingAccount>,

//     pub system_program: Program<'info, System>,
//     pub token_program: Program<'info, Token>,
//     pub rent: Sysvar<'info, Rent>,
// }

pub fn stake(ctx: Context<Stake>, stake_amount: u64) -> Result<()> {
    let total_token = ctx.accounts.stake_token_vault.amount;
    let total_x_token = ctx.accounts.staking_account.total_x_token;
    let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

    // mint x tokens
    if total_token == 0 || total_x_token == 0 {
        ctx.accounts.staking_account.total_x_token = (ctx.accounts.staking_account.total_x_token
            as u128)
            .checked_add(amount as u128)
            .unwrap()
            .try_into()
            .unwrap();
        ctx.accounts.user_staking_account.x_token_amount =
            (ctx.accounts.user_staking_account.x_token_amount as u128)
                .checked_add(amount as u128)
                .unwrap()
                .try_into()
                .unwrap();
    } else {
        let what: u64 = (amount as u128)
            .checked_mul(total_x_token as u128)
            .unwrap()
            .checked_div(total_token as u128)
            .unwrap()
            .try_into()
            .unwrap();

        ctx.accounts.staking_account.total_x_token = (ctx.accounts.staking_account.total_x_token
            as u128)
            .checked_add(what as u128)
            .unwrap()
            .try_into()
            .unwrap();
        ctx.accounts.user_staking_account.x_token_amount =
            (ctx.accounts.user_staking_account.x_token_amount as u128)
                .checked_add(what as u128)
                .unwrap()
                .try_into()
                .unwrap();
    }

    //transfer the users tokens to the vault
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token::Transfer {
            from: ctx.accounts.token_from.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.token_from_authority.to_account_info(),
        },
    );
    token::transfer(cpi_ctx, amount)?;

    (&mut ctx.accounts.token_vault).reload()?;

    //plus user staking amount
    ctx.accounts.user_staking_account.amount = (ctx.accounts.user_staking_account.amount as u128)
        .checked_add(amount as u128)
        .unwrap()
        .try_into()
        .unwrap();

    let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

    emit!(PriceChange {
        old_step_per_xstep_e9: old_price.0,
        old_step_per_xstep: old_price.1,
        new_step_per_xstep_e9: new_price.0,
        new_step_per_xstep: new_price.1,
    });

    Ok(())
}

const E9: u128 = 1000000000;

pub fn get_price<'info>(
    vault: &Account<'info, TokenAccount>,
    staking: &Account<'info, StakingAccount>,
) -> (u64, String) {
    let total_token = vault.amount;
    let total_x_token = staking.total_x_token;

    if total_x_token == 0 {
        return (0, String::from("0"));
    }

    let price_uint = (total_token as u128)
        .checked_mul(E9 as u128)
        .unwrap()
        .checked_div(total_x_token as u128)
        .unwrap()
        .try_into()
        .unwrap();
    let price_float = (total_token as f64) / (total_x_token as f64);
    return (price_uint, price_float.to_string());
}
