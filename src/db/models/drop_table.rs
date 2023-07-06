#![allow(clippy::module_name_repetitions)] // no.

pub struct DropTable {
    guild_id: String,
    drop_table_name: String,
    drop_table_parts: Vec<DropTablePart>,
}
pub struct DropTablePart {
    guild_id: String,
    drop_table_name: String,
    drop: DropTablePartOption,
    weight: i64,
}
pub enum DropTablePartOption {
    Item {
        item_name: String,
    },
    Currency {
        currency_name: String,
    },
}
