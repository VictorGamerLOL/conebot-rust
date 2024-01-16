# ConeBot

A discord economy management bot with the unique feature of supporting the handling of multiple currencies at once for one guild.
These currencies can also be setup by the guild staff to be exchangeable between each other with configurable rates.

---

This bot aims to make everything highly configurable and be as feature packed as possible to act as a replacement for many bots. The bot supports at the moment:

- [x] Currencies
  - [x] Create
  - [x] Delete
  - [x] Per-guild per-currency member balances
  - [x] Display member balances
  - [x] Give to members
  - [x] Take from members
  - [x] Changing config values
  - [x] Chat earning
  - [x] Whitelisting and blacklisting earning
    - [x] Channels
    - [x] Roles
  - [x] Exchanging between currencies
- [ ] Items
  - [x] Trophies
  - [ ] Consumables
    - [x] Message
    - [x] Role giving
    - [ ] Lootboxes (must complete below)
  - [ ] Instant consumables
    - [ ] Same as above
  - [x] Changing config values
  - [x] Inventories for each member
  - [ ] Sell to the void
- [ ] Lootboxes
  - [x] Create loot tables
  - [x] Delete loot tables
  - [ ] Make loot tables generate drops
    - [ ] Of items
    - [ ] Of currencies
  - [x] Associate loot tables to lootboxes
- [ ] Shops
  - [ ] Make listings
  - [ ] Take currency in exchange for item

---

## Running

### Requirements

- MongoDB
- Rust

To run the bot, you must do the following on your server:

- Clone the repository and cd into it

 ```bash
 git clone 'https://github.com/VictorGamerLOL/conebot-rust';
 cd conebot-rust
 ```

- Make a .env file with the following:

```env
TOKEN = # Your discord bot token here.
MONGO_URI = # Your MongoDB cluster URI here.
MONGO_DB =  # What is the MongoDB database called.
```

***⚠️⚠️Make sure your MongoDB cluster has replication enabled. Otherwise anything that uses transactions will NOT work.⚠️⚠️***

- Build the bot for release:

```bash
cargo build --release
```

- Run and enjoy. (Make sure MongoDB is running)

```bash
./target/release/conebot-rust
```

---

## Note

This also serves as my school project so there may be excessive comments in the code.
