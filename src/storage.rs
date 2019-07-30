
pub mod config_loader {
    use std::collections::HashMap;
    use std::error::Error;
    // load configs from the current directory
    // SECRET_KEY will hold the KEY
    // config.json will hold the configs of the bot
    
    #[allow(dead_code)]
    pub(crate) fn get_key() -> Result<String, Box<Error>> {
        // TODO: implement the function
        Ok("placeholder for the key".to_string())
    }

    #[allow(dead_code)]
    pub(crate) fn get_config() -> HashMap<String, String> {
        // TODO: implement the function
        HashMap::new()
    }

    // all other functions will be here as helpers and likely won't be exposed
}

pub mod data_loader {
    // TODO: loading data from json file(s)
    // ? probably in future we could encrypt and securely store the data
}

pub mod data_saver {
    // TODO: save data to json file(s)
}