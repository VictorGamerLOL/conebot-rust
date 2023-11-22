pub mod balance;
pub mod config;
pub mod currency;
pub mod give;
pub mod ping;
pub mod take;
pub mod test1;

//TODO: Reorganize commands because you cannot have individual permissions for subcommands.
// This means that if a member has access to basic commands such as balance they have access
// to the moderator only command. That is not a very good idea.
