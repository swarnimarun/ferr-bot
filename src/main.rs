use ferr_bot::{
    control::{Control, Identity},
    inventory::Inventory,
    store::Store,
    telegram::Telegram,
};
use std::{env, fs, process};
fn main() {
    if let Err(error) = run() {
        eprintln!("ferr-bot: {error}");
        process::exit(1);
    }
}
fn run() -> Result<(), &'static str> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.as_slice() == ["demo"] {
        let inventory = Inventory::parse(include_str!("../config/inventory.example.json"))?;
        let mut control = Control::new(
            42,
            inventory,
            Store::open(":memory:").map_err(|_| "Cannot initialize demo state")?,
        )?;
        let who = Identity {
            actor: 42,
            chat: 42,
            private: true,
        };
        println!("{}", control.command(who, 1, "/digest", 100).unwrap().text);
        let request = control.command(who, 2, "/restart media", 100).unwrap();
        println!("{}", request.text);
        println!(
            "{}",
            control
                .callback(who, &request.approve.unwrap(), 101)
                .unwrap()
                .text
        );
        println!("{}", control.command(who, 3, "/audit", 102).unwrap().text);
        return Ok(());
    }
    if args.as_slice() != ["telegram"] {
        return Err("usage: ferr-bot demo | telegram (see README)");
    }
    let owner = env::var("FERR_OWNER_ID")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|id| *id > 0)
        .ok_or("FERR_OWNER_ID must be a positive numeric user id")?;
    let token = env::var("TELEGRAM_BOT_TOKEN").map_err(|_| "TELEGRAM_BOT_TOKEN is required")?;
    let inventory_path =
        env::var("FERR_INVENTORY").unwrap_or_else(|_| "config/inventory.example.json".into());
    let metadata = fs::metadata(&inventory_path).map_err(|_| "Cannot read inventory")?;
    if metadata.len() > 65536 {
        return Err("Inventory too large");
    }
    let inventory = Inventory::parse(
        &fs::read_to_string(inventory_path).map_err(|_| "Cannot read inventory")?,
    )?;
    let db = env::var("FERR_DB").unwrap_or_else(|_| "state/ferr-bot.sqlite".into());
    let mut control = Control::new(
        owner,
        inventory,
        Store::open(db)
            .map_err(|_| "Cannot open state database; create private state directory first")?,
    )?;
    Telegram::new(token)?.run(&mut control)
}
