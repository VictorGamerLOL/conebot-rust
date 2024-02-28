pub mod balances;
pub mod currency;
pub mod drop_table;
pub mod inventory;
pub mod item;
pub mod store;

use anyhow::{ anyhow, Result };
pub use balances::{ Balance, Balances };
pub use currency::Currency;
pub use drop_table::DropTable;
pub use inventory::{ Inventory, InventoryEntry };
pub use item::{ Item, ItemError };
use serde::Serialize;
use serde_json::Value;
pub use store::StoreEntry;

/// This trait exists to serialize any struct that implements
/// serialize into pairs of strings, representing the field names and values.
///
/// This is used to display the information to the user in a readable
/// format, such as in the `balance` command. It is up the code that
/// receives these pairs to make them more readable, such as replacing
/// underscores with spaces.
pub trait ToKVs: Serialize {
    /// Tries to serialize itself into a JSON object,
    /// then deserialize itself into `Result<Vec<(String, String)>>`.
    ///
    /// # Errors
    /// This will return an error if the value provided
    /// cannot be interpreted as a json object, such as a
    /// regular string, number or `Vec`.
    fn try_to_kvs(&self) -> Result<Vec<(String, String)>> {
        match serde_json::to_value(self)? {
            Value::Object(o) =>
                Ok(
                    o
                        .into_iter()
                        .map(|(k, v)| (k, v.to_string()))
                        .collect()
                ),
            _ => Err(anyhow!("Could not convert to json object.")),
        }
    }
}
