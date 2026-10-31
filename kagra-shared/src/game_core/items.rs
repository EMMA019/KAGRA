//! Things a player carries: item kinds (JSON), a bag of stacks, coins, equipment slots that
//! add up to stats, and shops that buy and sell. Everything saves to JSON for `page.save`.
//!
//! ```text
//! let book = ItemBook::from_value(&json)?;      // the item kinds
//! let mut bag = Bag::new(20);
//! bag.add(&book, "potion", 3);
//! let mut gear = Gear::default();
//! gear.equip(&book, &mut bag, "iron_sword")?;   // out of the bag, into the hand
//! let atk = gear.stat(&book, "atk");
//! shop.buy(&book, 0, &mut bag, &mut coins)?;
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// What an item is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Used up (a potion): `heal`, `effect`.
    #[default]
    Consumable,
    /// Worn in a slot (`slot`): adds `stats`.
    Equipment,
    /// A key or a quest thing: never sold, never used up.
    Key,
    /// Sold or crafted with (an ore).
    Material,
}

/// One kind of item.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub name_ja: String,
    pub desc: String,
    pub desc_ja: String,
    pub kind: Kind,
    /// One glyph for lists ("⚔", "🛡", "♥"), or a short word.
    pub icon: String,
    /// The icon's colour in lists.
    pub color: [u8; 3],
    /// Price in a shop (selling gives half).
    pub price: u32,
    /// Most of one kind in a bag slot.
    pub stack: u32,
    /// For equipment: the slot it goes in ("hand", "body", "charm").
    pub slot: String,
    /// For equipment: what it adds ("atk": 3, "def": 1, "hp": 10).
    pub stats: BTreeMap<String, i32>,
    /// For a consumable: hit points it gives back.
    pub heal: i32,
    /// For a consumable: a word the kit acts on ("antidote", "warp").
    pub effect: String,
}

impl ItemDef {
    pub fn name(&self, ja: bool) -> &str {
        if ja && !self.name_ja.is_empty() {
            &self.name_ja
        } else {
            &self.name
        }
    }

    pub fn desc(&self, ja: bool) -> &str {
        if ja && !self.desc_ja.is_empty() {
            &self.desc_ja
        } else {
            &self.desc
        }
    }

    /// Selling gives half the price (rounded down); keys cannot be sold.
    pub fn sell_price(&self) -> Option<u32> {
        (self.kind != Kind::Key && self.price > 0).then_some(self.price / 2)
    }
}

/// Every item kind a game has, by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemBook {
    pub items: BTreeMap<String, ItemDef>,
}

impl ItemBook {
    /// From a JSON list of [`ItemDef`]s.
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let list: Vec<ItemDef> =
            serde_json::from_value(v.clone()).map_err(|e| format!("items: {e}"))?;
        Self::from_list(list)
    }

    pub fn from_list(list: Vec<ItemDef>) -> Result<Self, String> {
        let mut items = BTreeMap::new();
        for mut d in list {
            if d.id.is_empty() {
                return Err("items: an item has no id".into());
            }
            if d.kind == Kind::Equipment && d.slot.is_empty() {
                return Err(format!("items: {} is equipment with no slot", d.id));
            }
            d.stack = d.stack.max(1);
            let id = d.id.clone();
            if items.insert(id.clone(), d).is_some() {
                return Err(format!("items: {id} twice"));
            }
        }
        Ok(Self { items })
    }

    pub fn get(&self, id: &str) -> Option<&ItemDef> {
        self.items.get(id)
    }
}

/// So many of one item in one bag slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    pub id: String,
    pub count: u32,
}

/// Why a bag, a gear or a shop said no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    UnknownItem,
    BagFull,
    NotInBag,
    NotEquipment,
    /// Not a consumable.
    NotUsable,
    NotEnoughCoins,
    SoldOut,
    CannotSell,
}

/// The items a player carries: up to `slots` stacks, plus coins.
#[derive(Clone, Debug, PartialEq)]
pub struct Bag {
    pub stacks: Vec<Stack>,
    pub slots: usize,
    pub coins: u32,
}

impl Bag {
    pub fn new(slots: usize) -> Self {
        Self {
            stacks: Vec::new(),
            slots: slots.max(1),
            coins: 0,
        }
    }

    pub fn count(&self, id: &str) -> u32 {
        self.stacks
            .iter()
            .filter(|s| s.id == id)
            .map(|s| s.count)
            .sum()
    }

    pub fn has(&self, id: &str) -> bool {
        self.count(id) > 0
    }

    /// How many of `n` would fit.
    pub fn room_for(&self, book: &ItemBook, id: &str, n: u32) -> u32 {
        let Some(d) = book.get(id) else {
            return 0;
        };
        let top_up: u32 = self
            .stacks
            .iter()
            .filter(|s| s.id == id)
            .map(|s| d.stack.saturating_sub(s.count))
            .sum();
        let free = self.slots.saturating_sub(self.stacks.len()) as u32;
        n.min(top_up.saturating_add(free.saturating_mul(d.stack)))
    }

    /// Put `n` in: tops up stacks, then starts new ones. All or nothing.
    pub fn add(&mut self, book: &ItemBook, id: &str, n: u32) -> Result<(), Refusal> {
        let d = book.get(id).ok_or(Refusal::UnknownItem)?;
        if self.room_for(book, id, n) < n {
            return Err(Refusal::BagFull);
        }
        let mut left = n;
        for s in self.stacks.iter_mut().filter(|s| s.id == id) {
            let k = left.min(d.stack.saturating_sub(s.count));
            s.count += k;
            left -= k;
        }
        while left > 0 {
            let k = left.min(d.stack);
            self.stacks.push(Stack {
                id: id.into(),
                count: k,
            });
            left -= k;
        }
        Ok(())
    }

    /// Take `n` out (from the last stacks first). All or nothing.
    pub fn take(&mut self, id: &str, n: u32) -> Result<(), Refusal> {
        if self.count(id) < n {
            return Err(Refusal::NotInBag);
        }
        let mut left = n;
        for s in self.stacks.iter_mut().rev().filter(|s| s.id == id) {
            let k = left.min(s.count);
            s.count -= k;
            left -= k;
        }
        self.stacks.retain(|s| s.count > 0);
        Ok(())
    }

    /// Use one consumable: it leaves the bag and the kit gets its def to act on (heal,
    /// effect).
    pub fn use_one<'a>(&mut self, book: &'a ItemBook, id: &str) -> Result<&'a ItemDef, Refusal> {
        let d = book.get(id).ok_or(Refusal::UnknownItem)?;
        if d.kind != Kind::Consumable {
            return Err(Refusal::NotUsable);
        }
        self.take(id, 1)?;
        Ok(d)
    }

    /// Tidy: one list order by kind then name (for a menu).
    pub fn sort(&mut self, book: &ItemBook) {
        let rank = |id: &str| {
            book.get(id).map_or((9, String::new()), |d| {
                let k = match d.kind {
                    Kind::Equipment => 0,
                    Kind::Consumable => 1,
                    Kind::Material => 2,
                    Kind::Key => 3,
                };
                (k, d.name.clone())
            })
        };
        self.stacks.sort_by_key(|s| rank(&s.id));
    }
}

/// What a character wears: slot -> item id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gear {
    pub worn: BTreeMap<String, String>,
}

impl Gear {
    /// Move `id` from the bag into its slot; what was there goes back to the bag.
    pub fn equip(&mut self, book: &ItemBook, bag: &mut Bag, id: &str) -> Result<(), Refusal> {
        let d = book.get(id).ok_or(Refusal::UnknownItem)?;
        if d.kind != Kind::Equipment {
            return Err(Refusal::NotEquipment);
        }
        if !bag.has(id) {
            return Err(Refusal::NotInBag);
        }
        bag.take(id, 1)?;
        if let Some(old) = self.worn.insert(d.slot.clone(), id.into()) {
            if bag.add(book, &old, 1).is_err() {
                // No room for the old one: undo.
                self.worn.insert(d.slot.clone(), old);
                bag.add(book, id, 1).ok();
                return Err(Refusal::BagFull);
            }
        }
        Ok(())
    }

    /// Take off what is in `slot` into the bag.
    pub fn unequip(&mut self, book: &ItemBook, bag: &mut Bag, slot: &str) -> Result<(), Refusal> {
        let Some(id) = self.worn.get(slot).cloned() else {
            return Err(Refusal::NotInBag);
        };
        bag.add(book, &id, 1)?;
        self.worn.remove(slot);
        Ok(())
    }

    /// The sum of one stat over everything worn.
    pub fn stat(&self, book: &ItemBook, name: &str) -> i32 {
        self.worn
            .values()
            .filter_map(|id| book.get(id))
            .map(|d| d.stats.get(name).copied().unwrap_or(0))
            .sum()
    }

    /// How a stat would change if `id` replaced what is in its slot (for a shop or a menu:
    /// "+3 ATK").
    pub fn delta(&self, book: &ItemBook, id: &str, name: &str) -> i32 {
        let Some(d) = book.get(id) else {
            return 0;
        };
        let new = d.stats.get(name).copied().unwrap_or(0);
        let old = self
            .worn
            .get(&d.slot)
            .and_then(|o| book.get(o))
            .and_then(|o| o.stats.get(name).copied())
            .unwrap_or(0);
        new - old
    }
}

/// One line of a shop: an item, its price there (None = the book price), how many are left
/// (None = endless).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ware {
    pub id: String,
    pub price: Option<u32>,
    pub left: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Shop {
    pub name: String,
    pub name_ja: String,
    pub wares: Vec<Ware>,
}

impl Shop {
    pub fn price(&self, book: &ItemBook, i: usize) -> Option<u32> {
        let w = self.wares.get(i)?;
        Some(w.price.unwrap_or(book.get(&w.id)?.price))
    }

    /// Buy one of ware `i`.
    pub fn buy(&mut self, book: &ItemBook, i: usize, bag: &mut Bag) -> Result<(), Refusal> {
        let price = self.price(book, i).ok_or(Refusal::UnknownItem)?;
        let (id, left) = (self.wares[i].id.clone(), self.wares[i].left);
        if left == Some(0) {
            return Err(Refusal::SoldOut);
        }
        if bag.coins < price {
            return Err(Refusal::NotEnoughCoins);
        }
        bag.add(book, &id, 1)?;
        bag.coins -= price;
        if let Some(l) = self.wares[i].left.as_mut() {
            *l -= 1;
        }
        Ok(())
    }

    /// Sell one `id` from the bag for half its price.
    pub fn sell(book: &ItemBook, id: &str, bag: &mut Bag) -> Result<u32, Refusal> {
        let d = book.get(id).ok_or(Refusal::UnknownItem)?;
        let price = d.sell_price().ok_or(Refusal::CannotSell)?;
        bag.take(id, 1)?;
        bag.coins = bag.coins.saturating_add(price);
        Ok(price)
    }
}

/// The bag and the gear as JSON (for `page.save`), and back. Unknown items in a save are
/// dropped (a game that removed an item still loads).
pub fn save(bag: &Bag, gear: &Gear) -> Value {
    json!({
        "coins": bag.coins,
        "bag": bag.stacks.iter().map(|s| json!([s.id, s.count])).collect::<Vec<_>>(),
        "gear": gear.worn,
    })
}

pub fn load(book: &ItemBook, v: &Value, bag: &mut Bag, gear: &mut Gear) {
    bag.stacks.clear();
    bag.coins = v.get("coins").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
    if let Some(list) = v.get("bag").and_then(|b| b.as_array()) {
        for e in list {
            let id = e.get(0).and_then(|x| x.as_str()).unwrap_or("");
            let n = e.get(1).and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            if n > 0 {
                bag.add(book, id, n).ok();
            }
        }
    }
    gear.worn.clear();
    if let Some(m) = v.get("gear").and_then(|g| g.as_object()) {
        for (slot, id) in m {
            if let Some(d) = id.as_str().and_then(|id| book.get(id)) {
                if d.kind == Kind::Equipment && &d.slot == slot {
                    gear.worn.insert(slot.clone(), d.id.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> ItemBook {
        ItemBook::from_value(&json!([
            {"id": "potion", "name": "Potion", "name_ja": "やくそう", "icon": "♥", "price": 10, "stack": 5, "heal": 20},
            {"id": "sword", "name": "Iron Sword", "kind": "equipment", "slot": "hand", "price": 60, "stats": {"atk": 4}},
            {"id": "axe", "name": "Axe", "kind": "equipment", "slot": "hand", "price": 90, "stats": {"atk": 6, "def": -1}},
            {"id": "mail", "name": "Mail", "kind": "equipment", "slot": "body", "price": 80, "stats": {"def": 3}},
            {"id": "key", "name": "Crypt Key", "kind": "key"}
        ]))
        .unwrap()
    }

    #[test]
    fn a_book_is_strict() {
        assert!(ItemBook::from_value(&json!([{"id": "x", "kind": "equipment"}])).is_err());
        assert!(ItemBook::from_value(&json!([{"id": "x"}, {"id": "x"}])).is_err());
        assert!(ItemBook::from_value(&json!([{"id": "x", "colour": [1, 2, 3]}])).is_err());
        assert_eq!(book().get("potion").unwrap().name(true), "やくそう");
    }

    #[test]
    fn stacks_fill_then_the_bag_is_full() {
        let b = book();
        let mut bag = Bag::new(2);
        bag.add(&b, "potion", 7).unwrap();
        assert_eq!(bag.stacks.len(), 2);
        assert_eq!(bag.count("potion"), 7);
        assert_eq!(bag.room_for(&b, "potion", 9), 3);
        assert_eq!(bag.add(&b, "potion", 4), Err(Refusal::BagFull));
        assert_eq!(bag.count("potion"), 7, "all or nothing");
        assert_eq!(bag.add(&b, "sword", 1), Err(Refusal::BagFull));
        let d = bag.use_one(&b, "potion").unwrap();
        assert_eq!(d.heal, 20);
        bag.take("potion", 6).unwrap();
        assert!(bag.stacks.is_empty());
        assert_eq!(bag.take("potion", 1), Err(Refusal::NotInBag));
        assert_eq!(bag.add(&b, "ghost", 1), Err(Refusal::UnknownItem));
    }

    #[test]
    fn gear_swaps_through_the_bag_and_adds_up() {
        let b = book();
        let mut bag = Bag::new(10);
        let mut gear = Gear::default();
        bag.add(&b, "sword", 1).unwrap();
        bag.add(&b, "axe", 1).unwrap();
        bag.add(&b, "mail", 1).unwrap();
        gear.equip(&b, &mut bag, "sword").unwrap();
        gear.equip(&b, &mut bag, "mail").unwrap();
        assert_eq!((gear.stat(&b, "atk"), gear.stat(&b, "def")), (4, 3));
        assert_eq!(gear.delta(&b, "axe", "atk"), 2);
        assert_eq!(gear.delta(&b, "axe", "def"), -1);
        gear.equip(&b, &mut bag, "axe").unwrap();
        assert!(bag.has("sword"), "the sword went back to the bag");
        assert_eq!((gear.stat(&b, "atk"), gear.stat(&b, "def")), (6, 2));
        gear.unequip(&b, &mut bag, "hand").unwrap();
        assert_eq!(gear.stat(&b, "atk"), 0);
        assert_eq!(
            gear.equip(&b, &mut bag, "potion"),
            Err(Refusal::NotEquipment)
        );
        assert_eq!(gear.equip(&b, &mut bag, "mail"), Err(Refusal::NotInBag));
    }

    #[test]
    fn shops_sell_for_coins_run_out_and_buy_back_at_half() {
        let b = book();
        let mut shop: Shop = serde_json::from_value(json!({
            "name": "Smith",
            "wares": [{"id": "potion"}, {"id": "sword", "price": 50, "left": 1}]
        }))
        .unwrap();
        let mut bag = Bag::new(10);
        bag.coins = 70;
        assert_eq!(shop.price(&b, 1), Some(50));
        shop.buy(&b, 1, &mut bag).unwrap();
        assert_eq!((bag.coins, bag.count("sword")), (20, 1));
        assert_eq!(shop.buy(&b, 1, &mut bag), Err(Refusal::SoldOut));
        shop.buy(&b, 0, &mut bag).unwrap();
        shop.buy(&b, 0, &mut bag).unwrap();
        assert_eq!(shop.buy(&b, 0, &mut bag), Err(Refusal::NotEnoughCoins));
        assert_eq!(Shop::sell(&b, "sword", &mut bag), Ok(30));
        assert_eq!(bag.coins, 30);
        bag.add(&b, "key", 1).unwrap();
        assert_eq!(Shop::sell(&b, "key", &mut bag), Err(Refusal::CannotSell));
    }

    #[test]
    fn the_bag_and_gear_save_and_load() {
        let b = book();
        let mut bag = Bag::new(10);
        let mut gear = Gear::default();
        bag.coins = 42;
        bag.add(&b, "potion", 3).unwrap();
        bag.add(&b, "mail", 1).unwrap();
        gear.equip(&b, &mut bag, "mail").unwrap();
        let v = save(&bag, &gear);
        let mut bag2 = Bag::new(10);
        let mut gear2 = Gear::default();
        load(&b, &v, &mut bag2, &mut gear2);
        assert_eq!((bag2.coins, bag2.count("potion")), (42, 3));
        assert_eq!(gear2.worn.get("body").map(String::as_str), Some("mail"));
        // A save with an item the game no longer has, or gear in the wrong slot.
        load(
            &b,
            &json!({"coins": 5, "bag": [["gone", 2], ["potion", 1]], "gear": {"hand": "mail"}}),
            &mut bag2,
            &mut gear2,
        );
        assert_eq!((bag2.coins, bag2.stacks.len()), (5, 1));
        assert!(gear2.worn.is_empty());
    }
}
