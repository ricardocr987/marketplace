use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint, Transfer, CloseAccount};
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod marketplace {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        sol_amount: u64,
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;

        escrow.maker = ctx.accounts.authority.key();
        escrow.nft_mint = ctx.accounts.nft_mint.key();
        escrow.sol_mint = ctx.accounts.sol_mint.key();
        escrow.sol_amount = sol_amount;

        // Bump seeds found during constraint validation. We dont need to pass it via arguments and handlers dont have to recalculate it
        escrow.escrow_bump = *ctx.bumps.get("escrow").unwrap(); 
        escrow.vault_bump = *ctx.bumps.get("vault").unwrap();

        // Transfer from maker token account to vault via token_program CPI
        let cpi_accounts = Transfer {
            from: ctx.accounts.nft_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );
        token::transfer(cpi_ctx, 1)?;

        Ok(())
    }

    pub fn cancel(
        ctx: Context<Cancel>,
    ) -> Result<()> {
        // As the vault authority is the escrow account we need to define its seeds to sign the tx
        let nft_mint = ctx.accounts.escrow.nft_mint.key();
        let seeds = &[
            b"escrow",
            nft_mint.as_ref(),
            &[ctx.accounts.escrow.escrow_bump]
        ];
        let signer = &[&seeds[..]];

        // Transfer from vault to maker token account
        let cpi_accounts_tx = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.nft_token_account.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx_tx = CpiContext::new_with_signer(
            ctx.accounts.nft_token_account.to_account_info(), 
            cpi_accounts_tx, 
            signer
        );

        token::transfer(cpi_ctx_tx, 1)?;

        // Vault account close, as it is a tokenAccount we have to do it via CPI, the escrow account is closed via constraint in the context
        let cpi_accounts_close = CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.authority.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx_close = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(), 
            cpi_accounts_close, 
            signer,
        );
        token::close_account(cpi_ctx_close)?;

        Ok(())
    }

    pub fn exchange(
        ctx: Context<Exchange>,
    ) -> Result<()> {
        let nft_mint = ctx.accounts.escrow.nft_mint.key();
        let seeds = &[
            b"escrow",
            nft_mint.as_ref(),
            &[ctx.accounts.escrow.escrow_bump]
        ];
        let signer = &[&seeds[..]];
        // Transfer from taker token account to maker token account
        let cpi_accounts_to_maker = Transfer {
            from: ctx.accounts.token_account_taker_sol.to_account_info(),
            to: ctx.accounts.token_account_maker_sol.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(), // the authority in this instruction is the taker
        };
        let cpi_ctx_to_maker = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts_to_maker);
        token::transfer(cpi_ctx_to_maker, ctx.accounts.escrow.sol_amount)?;
       
        // Transfer from vault to taker token account of the tokens owned by the maker
        let cpi_accounts_to_taker = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.token_account_taker_nft.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx_to_taker = CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts_to_taker, signer);
        token::transfer(cpi_ctx_to_taker, ctx.accounts.vault.amount)?;
        
        // Vault account close, as it is a tokenAccount we have to do it via CPI, the escrow account is closed via constraint in the context
        let cpi_accounts_close = CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.maker.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx_close = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(), 
            cpi_accounts_close, 
            signer,
        );
        token::close_account(cpi_ctx_close)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 32 + 8 + 1 + 1, 
        seeds = [
            b"escrow",
            nft_mint.key().as_ref()
        ],
        bump,
    )]
    pub escrow: Account<'info, EscrowAccount>,
    #[account(
        init,
        payer = authority,
        seeds = [
            b"vault",
            escrow.key().as_ref(),
        ],
        bump,
        token::mint = nft_mint,
        token::authority = escrow,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        constraint = nft_token_account.mint == nft_mint.key()
    )]
    pub nft_token_account: Account<'info, TokenAccount>,
    #[account(
        constraint = nft_token_account.mint == nft_mint.key()
    )]
    pub nft_mint: Account<'info, Mint>,
    pub sol_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Cancel<'info> {
    #[account(
        mut,
        seeds = [
            b"escrow",
            escrow.nft_mint.key().as_ref()
        ],
        bump = escrow.escrow_bump,
        close = authority,
        constraint = *authority.key == escrow.maker
    )]
    pub escrow: Account<'info, EscrowAccount>,
    #[account(
        mut,
        seeds = [
            b"vault",
            escrow.key().as_ref(),
        ],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        constraint = nft_token_account.mint == escrow.nft_mint.key()
    )]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Exchange<'info> {
    #[account(
        mut,
        seeds = [
            b"escrow",
            escrow.nft_mint.key().as_ref()
        ],
        bump = escrow.escrow_bump,
        constraint = escrow.maker == *maker.key,
        close = maker,
    )]
    pub escrow: Account<'info, EscrowAccount>,
    #[account(
        mut,
        seeds = [
            b"vault",
            escrow.key().as_ref(),
        ],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>, // taker
    /// CHECK:
    #[account(
        mut, 
        constraint = escrow.maker == maker.key()
    )]/// CHECK:
    pub maker: AccountInfo<'info>, // el que crea el escrow
    #[account(
        mut,
        associated_token::mint = escrow.nft_mint,
        associated_token::authority = authority,
    )]
    pub token_account_taker_nft: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = token_account_maker_sol.mint == escrow.sol_mint
    )]
    pub token_account_maker_sol: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = token_account_taker_sol.mint == escrow.sol_mint
    )]
    pub token_account_taker_sol: Account<'info, TokenAccount>,
    #[account(constraint = mint_sol.key() == escrow.sol_mint)]
    pub mint_sol: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct EscrowAccount {
    pub maker: Pubkey,
    pub nft_mint: Pubkey,
    pub sol_mint: Pubkey,
    pub sol_amount: u64,
    pub escrow_bump: u8,
    pub vault_bump: u8
}
