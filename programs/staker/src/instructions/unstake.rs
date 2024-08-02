use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

#[derive(Accounts)]
#[instruction(nonce_vault: u8, _nonce_staking: u8, _nonce_user_staking: u8, amount: u64)]
pub struct Unstake<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    //the authority allowed to transfer from x_token_from
    pub x_token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = nonce_vault,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [ constants::STAKING_PDA_SEED.as_ref() ],
        bump = _nonce_staking,
        constraint = !staking_account.freeze_program,
    )]
    pub staking_account: ProgramAccount<'info, StakingAccount>,

    #[account(
        mut,
        seeds = [ x_token_from_authority.key().as_ref() ],
        bump = _nonce_user_staking,
        constraint = user_staking_account.x_token_amount >= amount
    )]
    pub user_staking_account: ProgramAccount<'info, UserStakingAccount>,

    #[account(mut)]
    //the token account to send token
    pub token_to: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn unstake(
    ctx: Context<Unstake>,
    nonce_vault: u8,
    _nonce_staking: u8,
    _nonce_user_staking: u8,
    amount: u64,
) -> ProgramResult {
    let now_ts = Clock::get().unwrap().unix_timestamp;
    let lock_end_date = ctx.accounts.staking_account.lock_end_date;

    if (now_ts as u64) < lock_end_date {
        return Err(ErrorCode::NotExceedLockEndDate.into());
    }

    let total_token = ctx.accounts.token_vault.amount;
    let total_x_token = ctx.accounts.staking_account.total_x_token;
    let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

    //burn what is being sent
    ctx.accounts.staking_account.total_x_token = (ctx.accounts.staking_account.total_x_token
        as u128)
        .checked_sub(amount as u128)
        .unwrap()
        .try_into()
        .unwrap();
    ctx.accounts.user_staking_account.x_token_amount =
        (ctx.accounts.user_staking_account.x_token_amount as u128)
            .checked_sub(amount as u128)
            .unwrap()
            .try_into()
            .unwrap();

    //determine user share of vault
    let what: u64 = (amount as u128)
        .checked_mul(total_token as u128)
        .unwrap()
        .checked_div(total_x_token as u128)
        .unwrap()
        .try_into()
        .unwrap();

    //compute vault signer seeds
    let token_mint_key = ctx.accounts.token_mint.key();
    let seeds = &[token_mint_key.as_ref(), &[nonce_vault]];
    let signer = &[&seeds[..]];

    //transfer from vault to user
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        token::Transfer {
            from: ctx.accounts.token_vault.to_account_info(),
            to: ctx.accounts.token_to.to_account_info(),
            authority: ctx.accounts.token_vault.to_account_info(),
        },
        signer,
    );
    token::transfer(cpi_ctx, what)?;

    (&mut ctx.accounts.token_vault).reload()?;

    //determine user staking amount
    let new_total_token = ctx.accounts.token_vault.amount;
    let new_total_x_token = ctx.accounts.staking_account.total_x_token;

    if new_total_token == 0 || new_total_x_token == 0 {
        ctx.accounts.user_staking_account.amount = 0;
    } else {
        let new_what: u64 = (ctx.accounts.user_staking_account.x_token_amount as u128)
            .checked_mul(new_total_token as u128)
            .unwrap()
            .checked_div(new_total_x_token as u128)
            .unwrap()
            .try_into()
            .unwrap();

        if new_what < ctx.accounts.user_staking_account.amount {
            ctx.accounts.user_staking_account.amount = new_what;
        }
    }

    let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.staking_account);

    emit!(PriceChange {
        old_step_per_xstep_e9: old_price.0,
        old_step_per_xstep: old_price.1,
        new_step_per_xstep_e9: new_price.0,
        new_step_per_xstep: new_price.1,
    });

    Ok(())
}
