

use telegram_bot::types::chat::User;


// handle message and return whether it could or couldn't properly handle the message
pub fn handle_text_messages<'a>(user : &User, message_data : &str) -> &'a str {
    // don't support non ascii for now
    if message_data.is_ascii() {
        "is ascii"
    } else {
        ""
    }
}


// to be noted that this is bad practice as it creates global variables but the scope is limited damn this is really awesome everytime I think about it.
pub fn handle_image_messages<'a>() -> &'a str {
    ""
}