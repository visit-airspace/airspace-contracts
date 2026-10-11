/// Associated Token Account PDA definition for external program reference.
/// This generates an `additionalPrograms` entry in the IDL with the ATA program's
/// PDA seeds, allowing instruction accounts to reference it via `pda("associatedToken", ...)`.
pub struct AssociatedToken;
