pub mod balance;
pub mod buy;
pub mod config_currency;
pub mod config_drop_table;
pub mod config_item;
pub mod config_store;
pub mod currency;
pub mod give;
pub mod inv;
pub mod ping;
pub mod sell;
pub mod take;
pub mod test1;
pub mod use_item;

//TODO: Reorganize commands because you cannot have individual permissions for subcommands.
// This means that if a member has access to basic commands such as balance they have access
// to the moderator only command. That is not a very good idea.
