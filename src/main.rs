mod storage;

mod updates;

use storage::config_loader;

extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

use updates::messages::message_handler;

use futures::Stream;
use tokio_core::reactor::Core;
use telegram_bot::*;

fn main() {
    // TODO: Add more than an example :P
    let mut core = Core::new().unwrap();

    let token = config_loader::get_key().unwrap();
    let api = Api::configure(token).build(core.handle()).unwrap();

    // Fetch new updates via long poll method
    let future = api.stream().for_each(|update| {

        // If the received update contains a new message...
        if let UpdateKind::Message(message) = update.kind {

            if let MessageKind::Text {ref data, ..} = message.kind {
                // Print received text message to stdout.
                let response = message_handler::handle_text_messages(&message.from, data);
                api.spawn(message.text_reply(response));
            }
        }

        Ok(())
    });

    core.run(future).unwrap();
}
