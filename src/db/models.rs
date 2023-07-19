pub mod balances;
pub mod currency;
mod drop_table;
pub mod global;
mod inventory;
mod item;
mod multipliers;
mod role_rewards;
mod store_entry;

pub use balances::{ Balance, Balances };
pub use currency::Currency;
pub use drop_table::DropTable;
pub use inventory::{ Inventory, InventoryEntry };
pub use item::Item;
pub use multipliers::{ Multiplier, Multipliers };
pub use role_rewards::{ RoleReward, RoleRewards };
pub use store_entry::StoreEntry;

use once_cell::sync::OnceCell;

use super::id::DbGuildId;

pub struct BotGuild {
    guild_id: DbGuildId,
    // Since we do not need to compute everything from the beginning as no command uses everything at once,
    // we use OnceCell to lazily initialize the data.
    pub(self) currencies: OnceCell<()>,
    pub(self) members: OnceCell<()>,
    pub(self) items: OnceCell<()>,
    pub(self) drop_tables: OnceCell<()>,
    pub(self) multipliers: OnceCell<()>,
    pub(self) global_currencies: OnceCell<()>,
}

impl BotGuild {
    pub fn new<T>(guild_id: T) -> Self where T: Into<DbGuildId> {
        Self {
            guild_id: guild_id.into(),
            currencies: OnceCell::new(),
            members: OnceCell::new(),
            items: OnceCell::new(),
            drop_tables: OnceCell::new(),
            multipliers: OnceCell::new(),
            global_currencies: OnceCell::new(),
        }
    }
}
