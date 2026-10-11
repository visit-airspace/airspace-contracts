use pinocchio::{account::AccountView, address::Address, error::ProgramError};
use pinocchio::{
    cpi::{Seed, Signer},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount, Transfer};

/// Close a PDA account and return the lamports to the recipient.
pub fn close_pda_account(pda_account: &AccountView, recipient: &AccountView) -> ProgramResult {
    if pda_account.address() == recipient.address() {
        return Err(ProgramError::InvalidArgument);
    }
    let payer_lamports = recipient.lamports();
    let recipient_balance =
        payer_lamports.checked_add(pda_account.lamports()).ok_or(ProgramError::ArithmeticOverflow)?;
    recipient.set_lamports(recipient_balance);
    pda_account.set_lamports(0);
    pda_account.close()?;
    assert_eq!(recipient.lamports(), recipient_balance, "closing the PDA lost the recipient's rent refund");
    assert_eq!(pda_account.data_len(), 0, "closed PDA retained account data");

    Ok(())
}

/// Create a PDA account for the given seeds.
///
/// Strict create-once semantics:
/// - If account has data: returns `AccountAlreadyInitialized`
/// - If account has lamports but no data: completes initialization via
///   transfer (if needed) + allocate + assign
/// - If account is fully absent (0 lamports): uses `CreateAccount`
pub fn create_pda_account<const N: usize>(
    payer: &AccountView,
    space: usize,
    owner: &Address,
    pda_account: &AccountView,
    pda_signer_seeds: [Seed; N],
) -> ProgramResult {
    let rent = Rent::get()?;

    let required_lamports = rent.try_minimum_balance(space)?.max(1);

    let signers = [Signer::from(&pda_signer_seeds)];

    if pda_account.lamports() > 0 {
        if pda_account.data_len() > 0 {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        // PDA was prefunded but not initialized yet.
        let additional_lamports = required_lamports.saturating_sub(pda_account.lamports());
        if additional_lamports > 0 {
            Transfer { from: payer, to: pda_account, lamports: additional_lamports }.invoke()?;
        }

        Allocate { account: pda_account, space: space as u64 }.invoke_signed(&signers)?;
        Assign { account: pda_account, owner }.invoke_signed(&signers)?;
    } else {
        CreateAccount { from: payer, to: pda_account, lamports: required_lamports, space: space as u64, owner }
            .invoke_signed(&signers)?;
    }
    assert_eq!(pda_account.data_len(), space, "created PDA has the wrong allocation size");
    assert!(pda_account.owned_by(owner), "created PDA was not assigned to its program");
    Ok(())
}

/// Create a PDA account idempotently for the given seeds.
///
/// **Security Warning**: This function allows re-initialization of existing accounts.
/// Use `create_pda_account` for strict "create once" semantics where re-init should error.
///
/// Use this for idempotent operations where re-initialization is acceptable.
/// If the account already exists and has data, it will be resized to the new space.
pub fn create_pda_account_idempotent<const N: usize>(
    payer: &AccountView,
    space: usize,
    owner: &Address,
    pda_account: &AccountView,
    pda_signer_seeds: [Seed; N],
) -> ProgramResult {
    let rent = Rent::get()?;

    let required_lamports = rent.try_minimum_balance(space)?.max(1);
    let original_lamports = pda_account.lamports();

    let signers = [Signer::from(&pda_signer_seeds)];

    if pda_account.lamports() > 0 {
        // Account exists - check if it needs resizing
        let current_len = pda_account.data_len();

        if current_len > 0 {
            // Account has data - use resize instead of Allocate
            if space > current_len {
                // Need to grow - first add lamports if needed
                let additional_lamports = required_lamports.saturating_sub(pda_account.lamports());
                if additional_lamports > 0 {
                    Transfer { from: payer, to: pda_account, lamports: additional_lamports }.invoke()?;
                }
                // Resize the account
                pda_account.resize(space)?;
            }
            // If space <= current_len, no action needed (data already fits)
        } else {
            // Account has lamports but no data (e.g., someone transferred lamports before init)
            let additional_lamports = required_lamports.saturating_sub(pda_account.lamports());
            if additional_lamports > 0 {
                Transfer { from: payer, to: pda_account, lamports: additional_lamports }.invoke()?;
            }
            Allocate { account: pda_account, space: space as u64 }.invoke_signed(&signers)?;
            Assign { account: pda_account, owner }.invoke_signed(&signers)?;
        }
    } else {
        CreateAccount { from: payer, to: pda_account, lamports: required_lamports, space: space as u64, owner }
            .invoke_signed(&signers)?;
    }
    assert!(pda_account.data_len() >= space, "idempotent PDA allocation does not fit the requested data");
    assert!(pda_account.lamports() >= original_lamports, "idempotent PDA allocation lost existing lamports");
    Ok(())
}

/// Resize an initialized PDA account to an exact size.
///
/// Ensures rent-exempt balance for growth and truncates for shrink, so callers
/// can rely on the account length matching `space` exactly.
pub fn resize_pda_account(payer: &AccountView, pda_account: &AccountView, space: usize) -> ProgramResult {
    if pda_account.data_len() == 0 {
        return Err(ProgramError::UninitializedAccount);
    }

    let rent = Rent::get()?;
    let required_lamports = rent.try_minimum_balance(space)?.max(1);
    let additional_lamports = required_lamports.saturating_sub(pda_account.lamports());
    if additional_lamports > 0 {
        Transfer { from: payer, to: pda_account, lamports: additional_lamports }.invoke()?;
    }

    if pda_account.data_len() != space {
        pda_account.resize(space)?;
    }
    assert_eq!(pda_account.data_len(), space, "resized PDA has the wrong allocation size");
    assert!(pda_account.lamports() >= required_lamports, "resized PDA is not funded for rent exemption");

    Ok(())
}
